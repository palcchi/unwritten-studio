#ifndef UNWRITTEN_STUDIO_CORE_H
#define UNWRITTEN_STUDIO_CORE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct UnwrittenStudioCore UnwrittenStudioCore;

/* A new local Session; iOS attempts wgpu/Metal initialization and retains CPU fallback. */
UnwrittenStudioCore *unwritten_studio_create(void);
void unwritten_studio_destroy(UnwrittenStudioCore *core);

/* Returns an owned UTF-8 JSON string (release with unwritten_studio_string_free). */
char *unwritten_studio_snapshot_json(const UnwrittenStudioCore *core);
char *unwritten_studio_execute(UnwrittenStudioCore *core, const char *command, const char *params_json);
char *unwritten_studio_new_project(UnwrittenStudioCore *core, const char *name);
char *unwritten_studio_load_project_json(UnwrittenStudioCore *core, const char *project_json);
char *unwritten_studio_serialize_project_json(const UnwrittenStudioCore *core);
char *unwritten_studio_import_media(UnwrittenStudioCore *core, const char *absolute_path);
char *unwritten_studio_set_time(UnwrittenStudioCore *core, double seconds);
char *unwritten_studio_undo(UnwrittenStudioCore *core);
char *unwritten_studio_redo(UnwrittenStudioCore *core);
void unwritten_studio_mark_saved(UnwrittenStudioCore *core);

/* Returns owned RGBA8 bytes. The caller frees exactly width*height*4 bytes with unwritten_studio_bytes_free. */
uint8_t *unwritten_studio_render_rgba8(const UnwrittenStudioCore *core, uint32_t max_side, uint32_t *width, uint32_t *height);
void unwritten_studio_bytes_free(uint8_t *bytes, size_t length);
void unwritten_studio_string_free(char *string);

/* Original desktop frontend host, not a separate native reconstruction.
 * Call every operation for a handle on one serial queue. Input JSON carries
 * viewport geometry, pointer/keyboard events and native file-dialog answers.
 * Returned pixels use the same owned RGBA8 allocation contract above.
 * Read output_json after every frame (also after a null pixel result) for
 * errors, clipboard/URL output, keyboard demand and native file-dialog requests.
 */
typedef struct UnwrittenStudioUI UnwrittenStudioUI;
typedef struct UnwrittenStudioAudio UnwrittenStudioAudio;
UnwrittenStudioUI *unwritten_studio_ui_create(void);
/* Owned UTF-8 diagnostic for a failed create; release using string_free. */
char *unwritten_studio_ui_last_error(void);
void unwritten_studio_ui_destroy(UnwrittenStudioUI *ui);
uint8_t *unwritten_studio_ui_frame(UnwrittenStudioUI *ui, const char *input_json,
                                 uint32_t *width, uint32_t *height);
char *unwritten_studio_ui_output_json(UnwrittenStudioUI *ui);

/* Callback-safe output handle. NativeUi owns the shared state and exposes a
 * live handle to Swift; request it on the editor's serial queue. Swift must
 * destroy it exactly once after stopping all AVAudioEngine callbacks. Valid
 * stopped pulls write silence; invalid arguments are rejected untouched. */
UnwrittenStudioAudio *unwritten_studio_ui_audio_handle(UnwrittenStudioUI *ui);
/* At most 96,000 interleaved stereo f32 samples at 48 kHz. */
size_t unwritten_studio_audio_pull(const UnwrittenStudioAudio *audio,
                                   float *interleaved_stereo, size_t sample_count);
/* Disjoint writable channels, at most 48,000 f32 frames per channel. */
size_t unwritten_studio_audio_pull_planar(const UnwrittenStudioAudio *audio,
                                         float *left, float *right, size_t frames);
void unwritten_studio_audio_destroy(UnwrittenStudioAudio *audio);

#ifdef __cplusplus
}
#endif
#endif
