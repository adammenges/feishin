#include <fileref.h>
#include <tbytevector.h>
#include <tpropertymap.h>
#include <tvariant.h>
#include <filesystem>
#include <memory>
#include <string>
#include <vector>
#include <exception>
#include <stdexcept>

struct NativeTags {
    TagLib::FileRef file;
    std::vector<std::pair<std::string, std::vector<std::string>>> fields;
    TagLib::ByteVector artwork;
    std::string error;
    explicit NativeTags(const char* filename)
        : file(TagLib::FileName(std::filesystem::u8path(filename).native().c_str()), false) {
        if (file.isNull() || !file.file()->isValid()) {
            error = "This audio file cannot be read by TagLib";
            return;
        }
        auto properties = file.properties();
        if (properties.size() > 1000) { error = "Audio file has too many metadata fields"; return; }
        size_t textSize = 0;
        for (const auto& entry : properties) {
            if (entry.second.size() > 10000) { error = "Audio metadata field has too many values"; return; }
            std::vector<std::string> values;
            for (const auto& value : entry.second) {
                auto utf8 = value.to8Bit(true);
                textSize += utf8.size();
                if (textSize > 20000000) { error = "Audio metadata is too large"; return; }
                values.push_back(std::move(utf8));
            }
            fields.emplace_back(entry.first.to8Bit(true), std::move(values));
        }
        for (const auto& picture : file.complexProperties("PICTURE")) {
            const auto data = picture.value("data").toByteVector();
            if (data.size() > 20000000) { error = "Cover artwork is too large"; return; }
            if (artwork.isEmpty() || picture.value("pictureType").toString() == "Front Cover") artwork = data;
            if (picture.value("pictureType").toString() == "Front Cover") break;
        }
    }
};
static thread_local std::string openError;
template<typename Function> static int guarded(NativeTags* tags, Function function) {
    try { function(); return 1; }
    catch (const std::exception& error) { tags->error = error.what(); return 0; }
    catch (...) { tags->error = "TagLib metadata operation failed"; return 0; }
}
extern "C" {
NativeTags* feishin_tags_open(const char* filename) {
    try { return new NativeTags(filename); }
    catch (const std::exception& error) { openError = error.what(); return nullptr; }
    catch (...) { openError = "TagLib could not open this audio file"; return nullptr; }
}
void feishin_tags_close(NativeTags* tags) { delete tags; }
const char* feishin_tags_error(const NativeTags* tags) { return tags ? tags->error.c_str() : openError.c_str(); }
size_t feishin_tags_count(const NativeTags* tags) { return tags->fields.size(); }
const char* feishin_tags_name(const NativeTags* tags, size_t index) { return index < tags->fields.size() ? tags->fields[index].first.c_str() : nullptr; }
size_t feishin_tags_values(const NativeTags* tags, size_t index) { return index < tags->fields.size() ? tags->fields[index].second.size() : 0; }
const char* feishin_tags_value(const NativeTags* tags, size_t index, size_t value) {
    return index < tags->fields.size() && value < tags->fields[index].second.size() ? tags->fields[index].second[value].c_str() : nullptr;
}
const unsigned char* feishin_tags_artwork(const NativeTags* tags, size_t* length) {
    *length = tags->artwork.size();
    return reinterpret_cast<const unsigned char*>(tags->artwork.data());
}
int feishin_tags_set(NativeTags* tags, const char* name, const char* const* values, size_t count) {
    return guarded(tags, [&] {
        const TagLib::String key(name, TagLib::String::UTF8);
        auto properties = tags->file.properties();
        properties.erase(key);
        if (count) {
            TagLib::StringList list;
            for (size_t index = 0; index < count; ++index) list.append(TagLib::String(values[index], TagLib::String::UTF8));
            properties.insert(key, list);
        }
        if (tags->file.setProperties(properties).contains(key)) throw std::runtime_error("This file format cannot preserve the selected metadata field");
    });
}
int feishin_tags_picture(NativeTags* tags, const unsigned char* bytes, size_t length, const char* mime) {
    return guarded(tags, [&] {
        TagLib::List<TagLib::VariantMap> pictures;
        if (length) {
            // The original editor replaces every picture. MP4 pictures also
            // have no pictureType field, so preserving them would leave the
            // old cover first and hide the replacement.
            TagLib::VariantMap picture;
            picture.insert("data", TagLib::ByteVector(reinterpret_cast<const char*>(bytes), static_cast<unsigned int>(length)));
            picture.insert("mimeType", TagLib::String(mime, TagLib::String::UTF8));
            picture.insert("pictureType", TagLib::String("Front Cover"));
            picture.insert("description", TagLib::String(""));
            pictures.append(picture);
        }
        if (!tags->file.setComplexProperties("PICTURE", pictures)) throw std::runtime_error("This file format cannot preserve cover artwork");
    });
}
int feishin_tags_save(NativeTags* tags) {
    return guarded(tags, [&] { if (!tags->file.save()) throw std::runtime_error("TagLib could not save this audio file"); });
}
}
