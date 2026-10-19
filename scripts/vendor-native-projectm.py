#!/usr/bin/env python3
"""Pin projectM's source and apply the native offscreen framebuffer fixes."""
import hashlib
import io
import pathlib
import shutil
import tarfile
import urllib.request

VERSION = "4.1.8"
SHA256 = "f72135e8b8e5fd807f908537255b41ac7ba4830309265e830cb756535487239e"
ROOT = pathlib.Path(__file__).resolve().parent.parent
DEST = ROOT / "crates/vendor/projectm-sys/libprojectM"

def replace(path, before, after):
    path = DEST / path
    text = path.read_text()
    assert text.count(before) == 1, f"Upstream changed: {path}"
    path.write_text(text.replace(before, after))

def patch():
    replace("src/libprojectM/ProjectM.hpp", "void RenderFrame();", "void RenderFrame(uint32_t targetFramebufferObject = 0);")
    replace("src/libprojectM/ProjectM.cpp", "void ProjectM::RenderFrame()", "void ProjectM::RenderFrame(uint32_t targetFramebufferObject)")
    replace("src/libprojectM/ProjectM.cpp",
            "// ToDo: Allow external apps to provide a custom target framebuffer.\n    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);",
            "// Backport projectM 4.2 framebuffer targeting for native offscreen rendering.\n    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, targetFramebufferObject);\n    glViewport(0, 0, m_windowWidth, m_windowHeight);")
    replace("src/libprojectM/ProjectMCWrapper.cpp", "void projectm_opengl_render_frame(projectm_handle instance)",
            """void projectm_opengl_render_frame_fbo(projectm_handle instance, uint32_t framebuffer_object_id)
{
    auto* projectMInstance = handle_to_instance(instance);
    projectMInstance->RenderFrame(framebuffer_object_id);
}

void projectm_opengl_render_frame(projectm_handle instance)""")
    replace("src/api/include/projectM-4/render_opengl.h", "PROJECTM_EXPORT void projectm_opengl_render_frame(projectm_handle instance);",
            "PROJECTM_EXPORT void projectm_opengl_render_frame(projectm_handle instance);\n\n/** Render to a caller-owned framebuffer. Backported from projectM 4.2. */\nPROJECTM_EXPORT void projectm_opengl_render_frame_fbo(projectm_handle instance, uint32_t framebuffer_object_id);")
    replace("src/libprojectM/Renderer/Framebuffer.cpp",
            "    UpdateDrawBuffers(framebufferIndex);\n\n    m_attachments.at(framebufferIndex).erase(attachmentType);",
            "    m_attachments.at(framebufferIndex).erase(attachmentType);\n    UpdateDrawBuffers(framebufferIndex);")
    path = DEST / "src/libprojectM/MilkdropPreset/BlurTexture.cpp"
    text = path.read_text()
    block = """    // Remember previously bound framebuffer
    GLint origReadFramebuffer;
    GLint origDrawFramebuffer;
    glGetIntegerv(GL_READ_FRAMEBUFFER_BINDING, &origReadFramebuffer);
    glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &origDrawFramebuffer);
"""
    assert text.count(block) == 1
    text = text.replace(block, "")
    path.write_text(text.replace("    AllocateTextures(sourceTexture);", block + "\n    AllocateTextures(sourceTexture);"))
    replace("src/libprojectM/Renderer/Texture.cpp",
            "    glTexImage2D(m_target, 0, m_internalFormat, m_width, m_height, 0, m_format, m_type, nullptr);",
            "    glTexImage2D(m_target, 0, m_internalFormat, m_width, m_height, 0, m_format, m_type, nullptr);\n    glTexParameteri(m_target, GL_TEXTURE_MIN_FILTER, GL_LINEAR);\n    glTexParameteri(m_target, GL_TEXTURE_MAG_FILTER, GL_LINEAR);")
    for filename in ("src/libprojectM/MilkdropPreset/MilkdropPresetExceptions.hpp", "src/libprojectM/Renderer/Shader.hpp"):
        path = DEST / filename
        text = path.read_text()
        path.write_text(text.replace("    const std::string& message() const", "    const char* what() const noexcept override { return m_message.c_str(); }\n\n    const std::string& message() const"))
    replace("src/libprojectM/MilkdropPreset/PerFrameContext.cpp", 'throw MilkdropCompileException("Could not compile per-frame code");',
            '''int errorLine{}, errorColumn{};
        auto* error = projectm_eval_get_error(perFrameCodeContext, &errorLine, &errorColumn);
        throw MilkdropCompileException(std::string("Could not compile per-frame code: ") + (error ? error : "unknown") + " line " + std::to_string(errorLine));''')

    replace("src/libprojectM/Renderer/TextureManager.cpp",
            "    // Return a placeholder.\n    return {m_placeholderTexture, m_samplers.at({wrapMode, filterMode}), name, unqualifiedName};",
            """    // Butterchurn uses its built-in cloud image for unresolved named textures.
    // Retain the original sampler name and wrap/filter settings for the shader.
    for (const auto& file : m_scannedTextureFiles)
    {
        if (file.lowerCaseBaseName == "clouds2")
        {
            auto texture = LoadTexture(file);
            if (texture)
            {
                return {texture, m_samplers.at({wrapMode, filterMode}), name, unqualifiedName};
            }
        }
    }
    return {m_placeholderTexture, m_samplers.at({wrapMode, filterMode}), name, unqualifiedName};""")
    replace("src/libprojectM/MilkdropPreset/MilkdropShader.cpp",
            "    size_t found;\n\n    // Find \"sampler_state\" overrides",
            """    // Legacy presets define noise1..noise4 with signatures different from GLSL's
    // reserved built-ins. Preserve their bodies/calls under an unreserved name.
    for (int dimension = 1; dimension <= 4; ++dimension)
    {
        auto name = std::string("noise") + std::to_string(dimension);
        if (std::regex_search(program, std::regex("float[1-4]?\\\\s+" + name + "\\\\s*\\\\(")))
        {
            program = std::regex_replace(program, std::regex("\\\\b" + name + "\\\\b"), "milkdrop_" + name);
        }
    }
    // The old shader language permits `sample` as a local identifier. The HLSL
    // parser reserves it as an interpolation modifier.
    program = std::regex_replace(program, std::regex("\\\\bsample\\\\b"), "milkdrop_sample");

    size_t found;

    // Find "sampler_state" overrides""")
    # Report invalid imported shaders through the existing preset failure callback.
    # A silent fallback can make the GPU suite pass while rendering the wrong effect.
    for filename in ("PerPixelMesh.cpp", "FinalComposite.cpp"):
        path = DEST / "src/libprojectM/MilkdropPreset" / filename
        text = path.read_text()
        start = text.index("void " + ("PerPixelMesh::CompileWarpShader" if filename == "PerPixelMesh.cpp" else "FinalComposite::CompileCompositeShader"))
        catch = text.index("        catch (Renderer::ShaderException& ex)", start)
        end = text.index("\n        }\n    }\n}", catch)
        text = text[:catch] + "        catch (Renderer::ShaderException&)\n        {\n            throw;" + text[end:]
        # LoadCode failures must also reach ProjectM's preset failure callback.
        catch = text.index("            catch (Renderer::ShaderException& ex)")
        end = text.index("\n            }", catch)
        text = text[:catch] + "            catch (Renderer::ShaderException&)\n            {\n                throw;" + text[end:]
        path.write_text(text)

def main():
    url = f"https://github.com/projectM-visualizer/projectm/releases/download/v{VERSION}/libprojectM-{VERSION}.tar.gz"
    with urllib.request.urlopen(url, timeout=60) as response:
        data = response.read()
    assert hashlib.sha256(data).hexdigest() == SHA256, "projectM source checksum mismatch"
    archive = tarfile.open(fileobj=io.BytesIO(data), mode="r:gz")
    shutil.rmtree(DEST)
    DEST.mkdir()
    for entry in archive:
        if not entry.isfile():
            continue
        relative = pathlib.PurePosixPath(entry.name).parts[1:]
        assert relative and ".." not in relative
        path = DEST.joinpath(*relative)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(archive.extractfile(entry).read())
    patch()
    print(f"Bundled projectM {VERSION}, SHA256 {SHA256}")

if __name__ == "__main__":
    main()
