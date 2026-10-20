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
UnwrittenStudioUI *unwritten_studio_ui_create(void);
/* Owned UTF-8 diagnostic for a failed create; release using string_free. */
char *unwritten_studio_ui_last_error(void);
void unwritten_studio_ui_destroy(UnwrittenStudioUI *ui);
uint8_t *unwritten_studio_ui_frame(UnwrittenStudioUI *ui, const char *input_json,
                                 uint32_t *width, uint32_t *height);
char *unwritten_studio_ui_output_json(UnwrittenStudioUI *ui);

#ifdef __cplusplus
}
#endif
#endif
