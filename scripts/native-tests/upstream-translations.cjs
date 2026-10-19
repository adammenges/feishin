// Refresh translation expectations from upstream's i18next, independently of the Rust formatter.
// npm install --prefix target/upstream-oracle --ignore-scripts i18next@25.10.10
// node scripts/native-tests/upstream-translations.cjs [base] [head]
const fs = require('node:fs');
const zlib = require('node:zlib');
const { execFileSync } = require('node:child_process');
const i18next = require('../../target/upstream-oracle/node_modules/i18next');
const root = 'crates/feishin-core/assets/';
const [base = '9dc15477', head = '3f321069'] = process.argv.slice(2);
const languages = fs.readdirSync(root + 'locales').map(file => file.replace('.json', ''));
async function engine(ref) {
    const resources = Object.fromEntries(languages.map(language => [language, { translation: JSON.parse(execFileSync('git', ['show', `${ref}:src/i18n/locales/${language}.json`])) }]));
    const instance = i18next.createInstance();
    const processors = {
        lowerCase: value => value.toLocaleLowerCase(),
        upperCase: value => value.toLocaleUpperCase(),
        titleCase: value => value.replace(/\S\S*/g, word => word.charAt(0).toLocaleUpperCase() + word.slice(1).toLowerCase()),
        sentenceCase: value => value.split('. ').map(sentence => sentence.charAt(0).toLocaleUpperCase() + sentence.slice(1).toLocaleLowerCase()).join('. '),
    };
    for (const [name, process] of Object.entries(processors)) instance.use({ name, type: 'postProcessor', process });
    await instance.init({ fallbackLng: 'en', resources, interpolation: { escapeValue: false } });
    return instance;
}
(async () => {
    const before = await engine(base), after = await engine(head);
    let updated = 0;
    function translate(expected, language, key, options = {}) {
        const previous = before.t(key, { ...options, lng: language });
        const next = after.t(key, { ...options, lng: language });
        if (previous === next) return expected;
        // Reruns are harmless, and only independently verified changes are accepted.
        if (expected !== previous && expected !== next) throw Error(`Unexpected reference: ${language} ${key}: ${expected} != ${previous}`);
        if (expected !== next) updated++;
        return next;
    }
    function refresh(file, update) {
        const path = root + file, compressed = file.endsWith('.gz');
        const bytes = fs.readFileSync(path);
        const original = (compressed ? zlib.gunzipSync(bytes) : bytes).toString();
        const data = JSON.parse(original), count = updated;
        update(data);
        if (count === updated) return;
        const text = JSON.stringify(data, null, compressed ? 0 : 2) + (compressed ? '' : '\n');
        fs.writeFileSync(path, compressed ? zlib.gzipSync(text, { level: 9 }) : text);
        console.log(`${file}: ${updated - count} translations refreshed`);
    }
    for (const name of ['i18n-reference.json.gz', 'i18n-context-reference.json.gz']) {
        refresh(name, cases => cases.forEach(c => c.expected = translate(c.expected, c.language, c.key, c.options)));
    }
    refresh('shortcut-labels-reference.json', data => {
        for (const [language, labels] of Object.entries(data.languages)) for (const id of Object.keys(labels.actions)) labels.actions[id] = translate(labels.actions[id], language, data.keys[id]);
    });
    refresh('smart-editor-reference.json', data => {
        for (const [language, kinds] of Object.entries(data.locales)) for (const entries of Object.values(kinds)) for (const entry of entries) entry.label = translate(entry.label, language, `filterOperator.${({gt:'isGreaterThan',lt:'isLessThan'})[entry.value] || entry.value}`);
    });
    refresh('artist-album-groups-reference.json', data => {
        for (const c of data.labels) {
            // Compare complete labels using the upstream release-type translations.
            const label = engine => c.key.split('/').map(key => {
                if (key === 'appears-on') return engine.t('page.albumArtistDetail.appearsOn', {lng:c.language});
                const translated = ['primary', 'secondary'].flatMap(group => Object.entries(engine.getResourceBundle('en', 'translation').releaseType[group]).map(([id]) => [`releaseType.${group}.${id}`, id]));
                const aliases = {'audio drama':'audioDrama','dj-mix':'djMix','field recording':'fieldRecording','mixtape/street':'mixtape','spokenword':'spokenWord'};
                const match = translated.find(([, id]) => id === (aliases[key] || key));
                const value = match ? engine.t(match[0], {lng:c.language}) : key.replace(/\S\S*/g, w => w[0].toUpperCase() + w.slice(1));
                return key === 'ep' ? value.toUpperCase() : value;
            }).join(' • ');
            const previous = label(before), next = label(after);
            if (previous !== next && c.label !== next) {
                if (c.label !== previous) throw Error(`Unexpected release label: ${JSON.stringify(c)}`);
                c.label = next; updated++;
            }
        }
    });
    refresh('metadata-surfaces-reference.json', data => {
        for (const c of data.fullInformation) for (const field of c.fields) field[3] = translate(field[3], c.language, field[0], {count:field[1]});
        for (const c of data.headers) for (const field of c.fields) {
            if (!['releaseDate','releaseYear'].includes(field[0])) continue;
            const key = 'page.albumDetail.released';
            const previous = before.t(key,{lng:c.language}), next = after.t(key,{lng:c.language});
            if (previous !== next && field[1].startsWith(previous + ' ')) { field[1] = next + field[1].slice(previous.length); updated++; }
        }
        for (const c of data.albumTags) for (const field of c.fields) {
            for (const [group, keys] of Object.entries(before.getResourceBundle('en','translation').releaseType)) for (const key of Object.keys(keys)) {
                const id = `releaseType.${group}.${key}`;
                if (field[1] === before.t(id,{lng:c.language})) field[1] = translate(field[1], c.language, id);
            }
        }
    });
    console.log(`${updated} expected labels updated from ${base} to ${head}`);
})().catch(error => { console.error(error); process.exitCode = 1; });
