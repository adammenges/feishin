// Execute the pinned analyzer's amplitude, mode and peak logic without browser/audio startup.
// FEISHIN_AUDIOMOTION_JS may point to audiomotion-analyzer 4.5.4 src/audioMotion-analyzer.js.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
const path = process.env.FEISHIN_AUDIOMOTION_JS || new URL(import.meta.resolve('audiomotion-analyzer'));
const source = readFileSync(path, 'utf8');
assert(source.includes("const VERSION = '4.5.4'"), 'Expected audioMotion 4.5.4 source');
const context = vm.createContext({ requestAnimationFrame: () => 0,
    window: { devicePixelRatio: 1, screen: { height: 1080 } },
    Path2D: class { rect() {} } });
vm.runInContext(source.replace('export { AudioMotionAnalyzer };', '')
    .replace('export default AudioMotionAnalyzer;', 'globalThis.Analyzer = AudioMotionAnalyzer;'), context);
function analyzer(options, width = 1280, height = 540, fps = 60) {
    const instance = Object.create(context.Analyzer.prototype);
    const canvas = { width, height };
    const defaults = { mode: 3, channelLayout: 'single', frequencyScale: 'log', minFreq: 20,
        maxFreq: 22050, mirror: 0, barSpace: .1, radius: .7, reflexRatio: .5, colorMode: 'gradient',
        minDecibels: -85, maxDecibels: -15, linearBoost: 4, gravity: 11, peakHoldTime: 500,
        peakFadeTime: 900, fillAlpha: 1, lineWidth: 1.9, showPeaks: true, ...options };
    for (const [key, value] of Object.entries(defaults)) instance['_' + key] = value;
    instance._chLayout = defaults.channelLayout;
    instance._ctx = new Proxy({ canvas }, { get: (obj, key) => key in obj ? obj[key] : () => {} });
    const node = { fftSize: 16384, frequencyBinCount: 8192,
        minDecibels: defaults.minDecibels, maxDecibels: defaults.maxDecibels,
        getFloatFrequencyData: data => data.fill(instance.decibels ?? -30) };
    Object.assign(instance, { _input: { context: { sampleRate: 48000 } },
        _analyzer: [node, node], _fftData: [new Float32Array(8192), new Float32Array(8192)],
        _ready: true, _createScales: () => {}, _pixelRatio: 1, _time: 0, _last: 0,
        _frames: 0, _fps: fps, _energy: { val: 0, peak: 0, hold: 0 }, _spinAngle: 0,
        _scaleX: { canvas: { height: 16 } }, _scaleR: { canvas: {} }, _canvasGradients: [],
        _selectedGrads: ['test', 'test'], _gradients: { test: { colorStops: [] } } });
    // These public properties have no setters/getters in the upstream implementation.
    instance.useCanvas = false;
    instance.showPeaks = defaults.showPeaks;
    instance.fillAlpha = defaults.fillAlpha;
    instance._calcBars();
    return instance;
}
const amplitudes = [];
for (const min of [-100, -85]) for (const max of [-30, -15, 0])
    for (const linear of [false, true]) for (const boost of [1, 2, 4]) {
        const options = { minDecibels: min, maxDecibels: max, linearAmplitude: linear, linearBoost: boost };
        const instance = analyzer(options);
        for (const decibels of [-200, min, -60, -40, max, 10])
            amplitudes.push({ options, decibels, expected: instance._normalizedB(decibels) });
    }
const modes = [];
for (const mode of [0, 3, 10]) for (const radial of [false, true])
    for (const lumiBars of [false, true]) for (const ledBars of [false, true]) {
        const options = { mode, radial, lumiBars, ledBars, alphaBars: true,
            outlineBars: true, roundBars: true, peakLine: true, showPeaks: true };
        const instance = analyzer(options);
        // The public ledBars setter stores the value in _showLeds.
        instance._showLeds = ledBars;
        instance._calcBars();
        modes.push({ options, flags: instance._flg, nominalHeight: radial ? Math.abs(instance._aux.outerRadius - instance._aux.innerRadius) : instance._aux.analyzerHeight });
    }
const peaks = [];
for (const fps of [30, 60, 120]) for (const height of [270, 1080])
    for (const mode of [3, 10]) for (const fadePeaks of [false, true]) for (const alphaBars of [false, true]) {
        const options = { mode, fadePeaks, alphaBars, peakLine: true, peakHoldTime: 0,
            peakFadeTime: 900, reflexRatio: 0, showPeaks: true };
        const instance = analyzer(options, 1280, height, fps);
        const frames = [];
        for (let frame = 1; frame <= fps / 2; frame++) {
            instance.decibels = frame === 1 ? -30 : -200;
            instance._draw(frame * 1000 / fps);
            const band = instance._bars[Math.floor(instance._bars.length / 2)];
            frames.push({ value: band.value[0], peak: band.peak[0], alpha: band.alpha[0] });
        }
        peaks.push({ options, size: [1280, height], fps, frames });
    }
const bands = [];
for (const mode of [0, 1, 2, 3, 4, 5, 6, 7, 8, 10])
    for (const frequencyScale of ['log', 'linear', 'bark', 'mel'])
        for (const ansiBands of [false, true]) for (const width of [320, 1280]) {
            const options = { mode, frequencyScale, ansiBands };
            const instance = analyzer(options, width);
            const rows = instance.getBars();
            const indices = [...new Set(Array.from({ length: 10 }, (_, i) => Math.round(i * (rows.length - 1) / 9)))];
            bands.push({ options, width, count: rows.length,
                expected: indices.map(index => ({ index, frequency: rows[index].freq,
                    low: rows[index].freqLo, high: rows[index].freqHi })) });
        }
const radial = [];
for (const channelLayout of ['single', 'dual-horizontal', 'dual-vertical', 'dual-combined'])
    for (const radialInvert of [false, true]) for (const radius of [.3, .7, 1]) {
        const options = { radial: true, channelLayout, radialInvert, radius };
        const instance = analyzer(options, 641, 481);
        radial.push({ options, size: [641, 481], inner: instance._aux.innerRadius,
            outer: instance._aux.outerRadius });
    }
const geometry = [];
for (const mode of [0, 1, 3, 8, 10]) for (const ledBars of [false, true])
    for (const channelLayout of ['single', 'dual-horizontal', 'dual-vertical', 'dual-combined'])
        for (const pixelRatio of [1, 2]) for (const reflexRatio of [0, .5]) for (const barSpace of [0, .1, 2]) {
            const options = { mode, ledBars, channelLayout, reflexRatio, barSpace };
            const instance = analyzer(options, 641, 481);
            instance._showLeds = ledBars;
            instance._pixelRatio = pixelRatio;
            instance._calcBars();
            const bars = instance._bars;
            const indices = [...new Set([0, 1, Math.floor(bars.length / 2), bars.length - 1])];
            geometry.push({ options, size: [641, 481], pixelRatio, aux: instance._aux,
                leds: instance._leds || null,
                bars: indices.map(index => ({ index, posX: bars[index].posX, width: bars[index].width })) });
        }
const definitions = vm.runInContext('JSON.parse(JSON.stringify(GRADIENTS))', context);
const gradients = {};
for (const [name, definition] of definitions) {
    const instance = analyzer({});
    instance.registerGradient(name, definition);
    gradients[name] = instance._gradients[name];
}
if (process.argv.includes('--gradients')) {
    process.stdout.write(JSON.stringify(gradients) + '\n');
    process.exit(0);
}
const colors = [];
for (const [name, definition] of Object.entries(gradients)) {
    for (const colorMode of ['bar-index', 'bar-level']) {
        for (const level of colorMode === 'bar-index' ? [.5] : [0, .1, .3, .475, .5, .7, .85, .9, 1]) {
            const instance = analyzer({ colorMode, showPeaks: false });
            instance._gradients[name] = definition;
            instance._selectedGrads = [name, name];
            instance.useCanvas = true;
            instance.overlay = true;
            const painted = [];
            instance._ctx.fill = () => painted.push(instance._ctx.fillStyle);
            instance.decibels = -85 + level * 70;
            instance._draw(1000 / 60);
            const actualLevel = instance._bars[0].value[0];
            for (let index = 0; index < (colorMode === 'bar-index' ? 15 : 1); index++)
                colors.push({ name, colorMode, level: actualLevel, index, expected: painted[index] });
        }
    }
}
const polygons = [];
for (const channelLayout of ['single', 'dual-horizontal', 'dual-vertical', 'dual-combined'])
    for (const mirror of [-1, 0, 1]) for (const radialInvert of [false, true]) for (const roundBars of [false, true]) {
        const options = { mode: 3, barSpace: .1, radius: .7, radial: true, channelLayout, mirror, radialInvert, roundBars, showPeaks: false };
        const instance = analyzer(options, 641, 481);
        const count = instance._bars.length;
        instance._bars = instance._bars.slice(0, 1);
        instance._spinAngle = -Math.PI / 2;
        instance.useCanvas = true;
        instance.overlay = true;
        let points = [];
        const paths = [];
        instance._ctx.beginPath = () => { points = []; };
        instance._ctx.moveTo = instance._ctx.lineTo = (...point) => points.push(point);
        instance._ctx.arc = (x, y, radius, start, end) => points.push([x + radius * Math.cos(end), y + radius * Math.sin(end)]);
        instance._ctx.fill = () => paths.push(points);
        instance._draw(1000 / 60);
        polygons.push({ options, size: [641, 481], count, value: instance._bars[0].value[0], paths });
    }
const reflections = [];
for (const channelLayout of ['single', 'dual-horizontal', 'dual-vertical', 'dual-combined'])
    for (const reflexFit of [false, true]) for (const reflexRatio of [.25, .5, .75]) {
        const options = { channelLayout, reflexFit, reflexRatio, reflexAlpha: .4, reflexBright: .7, showPeaks: false };
        const instance = analyzer(options, 641, 481);
        instance.useCanvas = true;
        instance.overlay = true;
        instance.reflexFit = reflexFit;
        instance.reflexAlpha = .4;
        instance.reflexBright = .7;
        const draws = [];
        let transform;
        instance._ctx.setTransform = (...values) => { transform = values; };
        instance._ctx.drawImage = (canvas, ...rectangle) => draws.push({ transform, rectangle, alpha: instance._ctx.globalAlpha, filter: instance._ctx.filter });
        instance._draw(1000 / 60);
        reflections.push({ options, size: [641, 481], draws });
    }
const scales = [];
for (const mode of [3, 10]) for (const frequencyScale of ['log', 'linear', 'bark', 'mel'])
    for (const noteLabels of [false, true]) for (const mirror of [-1, 0, 1])
        for (const channelLayout of ['single', 'dual-horizontal', 'dual-vertical']) for (const ansiBands of [false, true]) {
            const options = { mode, frequencyScale, noteLabels, mirror, channelLayout, ansiBands };
            const instance = analyzer(options, 641, 481);
            const labels = [];
            const axis = { canvas: { width: 641, height: 20 }, measureText: () => ({ width: 0 }) };
            axis.fillText = (text, x, y, maxWidth) => labels.push({ text: String(text), x, y, maxWidth, color: axis.fillStyle });
            instance._scaleX = new Proxy(axis, { get: (object, key) => key in object ? object[key] : () => {} });
            instance._scaleR = new Proxy({ canvas: {} }, { get: (object, key) => key in object ? object[key] : () => {} });
            context.Analyzer.prototype._createScales.call(instance);
            scales.push({ options, size: [641, 481], labels });
        }
const gradientFields = [];
const rgb = text => {
    const named = { red: '#ff0000', yellow: '#ffff00', lime: '#00ff00', OrangeRed: '#ff4500', SteelBlue: '#4682b4' };
    let hex = named[text] || text;
    if (hex.length === 4) hex = '#' + [...hex.slice(1)].map(value => value + value).join('');
    assert(/^#[\da-f]{6}$/i.test(hex), hex);
    return [1, 3, 5].map(index => parseInt(hex.slice(index, index + 2), 16) / 255);
};
for (const [name, definition] of Object.entries(gradients))
    for (const channelLayout of ['single', 'dual-horizontal', 'dual-vertical', 'dual-combined'])
        for (const radial of [false, true]) for (const lumiBars of [false, true]) for (const splitGradient of [false, true]) {
            const options = { mode: 3, gradient: name, gradientLeft: name, gradientRight: name, channelLayout, radial, lumiBars, splitGradient };
            const instance = analyzer(options, 641, 481);
            instance._selectedGrads = [name, name]; instance._gradients[name] = definition;
            const makeGradient = (type, points) => ({ type, points, stops: [], addColorStop(pos, color) { this.stops.push({ pos, color }); } });
            instance._ctx.createRadialGradient = (...points) => makeGradient('radial', points);
            instance._ctx.createLinearGradient = (...points) => makeGradient('linear', points);
            instance._makeGrad();
            for (let channel = 0; channel < (channelLayout === 'single' ? 1 : 2); channel++) for (const level of [0, .1, .3, .5, .8, 1]) {
                const gradient = instance._canvasGradients[channel];
                const aux = instance._aux;
                let pos;
                if (gradient.type === 'radial') {
                    const radius = aux.innerRadius + (aux.outerRadius - aux.innerRadius) * level * (channelLayout === 'dual-vertical' && channel ? -1 : 1);
                    pos = (radius - gradient.points[2]) / (gradient.points[5] - gradient.points[2]);
                } else if (gradient.points[0] !== gradient.points[2]) pos = .2;
                else pos = (aux.channelCoords[channel].channelTop + aux.analyzerHeight * (1 - level)) / gradient.points[3];
                const stops = [...gradient.stops].sort((a,b) => a.pos - b.pos);
                let expected = rgb(stops.at(-1).color);
                for (let index = 0; index < stops.length; index++) if (pos <= stops[index].pos) {
                    const right = stops[index], left = stops[Math.max(0,index-1)];
                    const fraction = right.pos === left.pos ? 1 : Math.max(0, Math.min(1, (pos-left.pos)/(right.pos-left.pos)));
                    expected = rgb(left.color).map((value, channel) => value + (rgb(right.color)[channel]-value)*fraction);
                    break;
                }
                gradientFields.push({ options, size: [641, 481], channel, index: .2, level, expected });
            }
        }
const scalesY = [];
for (const mode of [3, 10]) for (const channelLayout of ['single', 'dual-horizontal', 'dual-vertical', 'dual-combined'])
    for (const mirror of [-1, 0, 1]) for (const linearAmplitude of [false, true])
        for (const [minDecibels, maxDecibels] of [[-85,-15],[-100,0],[-42,-3]]) {
            const options = { mode, channelLayout, mirror, linearAmplitude, minDecibels, maxDecibels, showScaleY: true, showPeaks: false };
            const instance = analyzer(options, 641, 481);
            instance.useCanvas = true; instance.overlay = true; instance.showScaleY = true;
            instance._scaleX.canvas.height = 20;
            const labels = [], lines = [];
            let points, dash;
            instance._ctx.fillText = (text, x, y) => labels.push({ text: String(text), x, y });
            instance._ctx.setLineDash = values => { dash = values; };
            instance._ctx.beginPath = () => { points = []; };
            instance._ctx.moveTo = instance._ctx.lineTo = (...point) => points.push(point);
            instance._ctx.stroke = () => {
                if (['#888','#555'].includes(instance._ctx.strokeStyle) && points.length === 2 && dash?.[0] === 2)
                    lines.push({ points, dash, offset: instance._ctx.lineDashOffset, color: instance._ctx.strokeStyle });
            };
            instance._draw(1000 / 60);
            scalesY.push({ options, size: [641, 481], labels, lines });
        }
const radialScales = [];
for (const mode of [3, 10]) for (const frequencyScale of ['log', 'linear', 'bark', 'mel'])
    for (const noteLabels of [false, true]) for (const mirror of [-1, 0, 1])
        for (const channelLayout of ['single', 'dual-horizontal', 'dual-vertical', 'dual-combined'])
            for (const ansiBands of [false, true]) for (const radialInvert of [false, true]) {
                const options = { mode, frequencyScale, noteLabels, mirror, channelLayout, ansiBands, radialInvert, radial: true };
                const instance = analyzer(options, 641, 481);
                const labels = [];
                let translate, angle;
                const axis = { canvas: {}, translate: (x,y) => { translate = [x,y]; }, rotate: value => { angle = value; } };
                axis.fillText = (text, x, y) => labels.push({ text: String(text), position: translate, angle, color: axis.fillStyle });
                instance._scaleX = new Proxy({ canvas: { width: 641, height: 20 }, measureText: () => ({ width: 0 }) }, { get: (object, key) => key in object ? object[key] : () => {} });
                instance._scaleR = new Proxy(axis, { get: (object, key) => key in object ? object[key] : () => {} });
                context.Analyzer.prototype._createScales.call(instance);
                radialScales.push({ options, size: [641, 481], diameter: axis.canvas.width, labels });
            }
assert.equal(amplitudes.length, 216);
assert.equal(modes.length, 24);
assert.equal(peaks.length, 48);
assert.equal(colors.length, 120);
assert.equal(geometry.length, 480);
assert.equal(polygons.length, 48);
assert.equal(reflections.length, 24);
assert.equal(scales.length, 288);
assert.equal(gradientFields.length, 1680);
assert.equal(scalesY.length, 144);
assert.equal(radialScales.length, 768);
assert(polygons.every(case_ => case_.paths.length > 0 && case_.paths.every(path => path.length >= 4)));
assert(reflections.every(case_ => case_.draws.length > 0 && case_.draws.every(draw => draw.rectangle.length === 8)));
assert(colors.every(color => typeof color.expected === 'string'));
process.stdout.write(JSON.stringify({ version: '4.5.4', amplitudes, modes, peaks, bands, radial, geometry, polygons, reflections, scales, scalesY, radialScales, gradientFields, gradients, colors }) + '\n');
