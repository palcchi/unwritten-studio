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

#ifdef __cplusplus
}
#endif
#endif
