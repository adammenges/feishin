/* Synthetic CMYK and YCCK JPEG fixtures, encoded by the independent libjpeg codec. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <jpeglib.h>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    FILE *profile_file = fopen(argv[1], "rb");
    if (!profile_file) return 3;
    fseek(profile_file, 0, SEEK_END);
    long profile_size = ftell(profile_file);
    if (profile_size <= 0 || profile_size > 16 * 1024 * 1024) return 4;
    rewind(profile_file);
    unsigned char *profile = malloc((size_t)profile_size);
    if (!profile || fread(profile, 1, (size_t)profile_size, profile_file) != (size_t)profile_size) return 5;
    fclose(profile_file);
    const unsigned char samples[24] = {
        40,100,150,20, 200,40,60,10, 100,20,200,80,
        0,0,0,0, 255,255,255,255, 120,80,60,90
    };
    unsigned char inverted[16 * 24 * 4];
    /* Solid 8x8 tiles avoid inverse-DCT rounding obscuring the ICC comparison. */
    for (int y = 0; y < 24; y++) {
        for (int x = 0; x < 16; x++) {
            int tile = (y / 8) * 2 + x / 8;
            for (int channel = 0; channel < 4; channel++)
                inverted[(y * 16 + x) * 4 + channel] = 255 - samples[tile * 4 + channel];
        }
    }
    for (int ycck = 0; ycck < 2; ycck++) {
        for (int tagged = 0; tagged < 2; tagged++) {
            char path[4096];
            int length = snprintf(path, sizeof(path), "%s/%s-%s.jpeg", argv[2], ycck ? "YCCK" : "CMYK", tagged ? "tagged" : "untagged");
            if (length < 0 || length >= (int)sizeof(path)) return 6;
            FILE *output = fopen(path, "wb");
            if (!output) return 7;
            struct jpeg_compress_struct compressor;
            struct jpeg_error_mgr error;
            compressor.err = jpeg_std_error(&error);
            jpeg_create_compress(&compressor);
            jpeg_stdio_dest(&compressor, output);
            compressor.image_width = 16;
            compressor.image_height = 24;
            compressor.input_components = 4;
            compressor.in_color_space = JCS_CMYK;
            jpeg_set_defaults(&compressor);
            jpeg_set_colorspace(&compressor, ycck ? JCS_YCCK : JCS_CMYK);
            jpeg_set_quality(&compressor, 100, TRUE);
            for (int i = 0; i < 4; i++) compressor.comp_info[i].h_samp_factor = compressor.comp_info[i].v_samp_factor = 1;
            jpeg_start_compress(&compressor, TRUE);
            if (tagged) {
                int chunks = (int)((profile_size + 65518) / 65519);
                for (int i = 0; i < chunks; i++) {
                    long offset = (long)i * 65519;
                    size_t size = (size_t)(profile_size - offset);
                    if (size > 65519) size = 65519;
                    unsigned char marker[65533];
                    memcpy(marker, "ICC_PROFILE\0", 12);
                    marker[12] = (unsigned char)(i + 1);
                    marker[13] = (unsigned char)chunks;
                    memcpy(marker + 14, profile + offset, size);
                    jpeg_write_marker(&compressor, JPEG_APP0 + 2, marker, (unsigned int)size + 14);
                }
            }
            while (compressor.next_scanline < compressor.image_height) {
                JSAMPROW row = inverted + compressor.next_scanline * 16 * 4;
                jpeg_write_scanlines(&compressor, &row, 1);
            }
            jpeg_finish_compress(&compressor);
            jpeg_destroy_compress(&compressor);
            fclose(output);
        }
    }
    free(profile);
    return 0;
}
