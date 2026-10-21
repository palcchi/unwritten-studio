/* Original clean-room Unwritten Studio AE importer. Payload is supplied by the Rust exporter.
 * No JSON global or eval required. Existing compositions are never closed or replaced.
 */
(function () {
    if (typeof app === "undefined" || !app.project) { alert("Open an After Effects project first."); return; }
    var data = __UNWRITTEN_PAYLOAD__, warnings = [], ids = {}, made = [], saveTo = null, saved = false;
    function finite(v) { return typeof v === "number" && isFinite(v); }
    function integer(v) { return finite(v) && Math.floor(v) === v; }
    function object(v) { return v !== null && typeof v === "object" && !(v instanceof Array); }
    function boundedString(v) { return typeof v === "string" && v.length <= 16384 && v.indexOf("\0") < 0; }
    function boundedTime(v) { return finite(v) && Math.abs(v) <= 86400; }
    function boundedNumber(v) { return finite(v) && Math.abs(v) <= 1000000000; }
    function color(v) { var i; return v instanceof Array && v.length === 3 && (function () { for (i = 0; i < 3; i++) if (!finite(v[i]) || v[i] < 0 || v[i] > 1) return false; return true; }()); }
    function dimensions(w, h) { return integer(w) && integer(h) && w >= 4 && w <= 30000 && h >= 4 && h <= 30000; }
    function validVector(v) { return v instanceof Array && v.length === 2 && boundedNumber(v[0]) && boundedNumber(v[1]); }
    function validProperty(p, name, layerStart, stretch) {
        var i, k, last = null;
        if (!object(p) || !(p.keys instanceof Array) || p.keys.length > 100000) return false;
        if (name === "rotation" || name === "opacity") { if (!boundedNumber(p.value)) return false; }
        else if (name === "anchor" || name === "position" || name === "scale") { if (!validVector(p.value)) return false; }
        else return false;
        for (i = 0; i < p.keys.length; i++) {
            k = p.keys[i];
            if (!object(k) || !boundedTime(k.time) || (last !== null && k.time <= last) || (k.hold !== true && k.hold !== false)) return false;
            if (!boundedTime((k.time - layerStart) * 100 / stretch)) return false;
            if (name === "rotation" || name === "opacity") { if (!boundedNumber(k.value)) return false; }
            else if (!validVector(k.value)) return false;
            last = k.time;
        }
        return true;
    }
    function compIndex(comps, id) { var i; for (i = 0; i < comps.length; i++) if (comps[i].id === id) return i; return -1; }
    function graphVisit(comps, index, stack, done, depth) {
        var i, j, source, child;
        if (depth > 64) return false;
        for (i = 0; i < done.length; i++) if (done[i] === index) return true;
        for (i = 0; i < stack.length; i++) if (stack[i] === index) return false;
        stack.push(index);
        for (j = 0; j < comps[index].layers.length; j++) if (comps[index].layers[j].kind === "comp") {
            source = comps[index].layers[j].source; child = compIndex(comps, source);
            if (child < 0 || !graphVisit(comps, child, stack, done, depth + 1)) return false;
        }
        stack.pop(); done.push(index); return true;
    }
    function validEnvelope() {
        var i, c, j, l, n, p, totalLayers = 0, totalKeys = 0, seen = [], names = ["anchor", "position", "scale", "rotation", "opacity"], done;
        if (!object(data) || data.format !== "unwritten-after-effects" || data.version !== 1 || !(data.comps instanceof Array) || !data.comps.length || data.comps.length > 512) return false;
        if (data.warnings !== undefined && (!(data.warnings instanceof Array) || data.warnings.length > 1000)) return false;
        for (i = 0; i < (data.warnings || []).length; i++) if (!boundedString(data.warnings[i])) return false;
        for (i = 0; i < data.comps.length; i++) {
            c = data.comps[i];
            // Keep this check independent from compIndex(), which expects full
            // composition objects and is also used for precomp references.
            if (!object(c) || !boundedString(c.id) || !c.id.length || seen.indexOf(c.id) >= 0 || !boundedString(c.name) || !dimensions(c.width, c.height) || !finite(c.pixelAspect) || c.pixelAspect < 0.1 || c.pixelAspect > 10 || !boundedTime(c.duration) || c.duration <= 0 || !finite(c.fps) || c.fps < 0.001 || c.fps > 1000 || !color(c.background) || !(c.layers instanceof Array)) return false;
            seen.push(c.id);
            totalLayers += c.layers.length; if (totalLayers > 20000) return false;
            for (j = 0; j < c.layers.length; j++) {
                l = c.layers[j];
                if (!object(l) || !boundedString(l.name) || !boundedString(l.source) || !boundedString(l.text) || !boundedString(l.font) || ["solid", "null", "text", "footage", "comp"].indexOf(l.kind) < 0 || !dimensions(l.width, l.height) || !color(l.color) || !finite(l.fontSize) || l.fontSize < 0.1 || l.fontSize > 10000 || !boundedTime(l.startTime) || !boundedTime(l.inPoint) || !boundedTime(l.outPoint) || l.outPoint <= l.inPoint || !finite(l.stretch) || l.stretch < 0.01 || l.stretch > 10000 || typeof l.enabled !== "boolean" || !object(l.transform)) return false;
                if ((l.kind === "footage" || l.kind === "comp") && !l.source.length) return false;
                for (n = 0; n < names.length; n++) { p = l.transform[names[n]]; if (!validProperty(p, names[n], l.startTime, l.stretch)) return false; totalKeys += p.keys.length; if (totalKeys > 100000) return false; }
            }
        }
        done = [];
        for (i = 0; i < data.comps.length; i++) if (!graphVisit(data.comps, i, [], done, 0)) return false;
        return true;
    }
    if (!validEnvelope()) { alert("Invalid or unsupported Unwritten AE interchange payload. No compositions were created."); return; }
    warnings = data.warnings instanceof Array ? data.warnings : [];
    if (!confirm("Create " + data.comps.length + " composition(s) in the CURRENT After Effects project?\nExisting content stays intact. For a clean AEP, start with an empty project. Only basic 2D features transfer; warnings describe omissions.")) return;
    function warn(s) { if (warnings.length < 999) warnings.push(String(s)); else if (warnings.length === 999) warnings.push("Additional warnings omitted."); }
    function summary() { var s = "", i; for (i = 0; i < warnings.length && i < 12; i++) s += "\n- " + warnings[i]; if (warnings.length > 12) s += "\n...and " + (warnings.length - 12) + " more."; return s; }
    var i, j, c, d, dest, nl, src, doc, st, undoOpen = false;
    function applyProperty(p, a) {
        if (!p || !a) return;
        var k, key, mode;
        if (!a.keys.length) { p.setValue(a.value); return; }
        for (k = 0; k < a.keys.length; k++) p.setValueAtTime(a.keys[k].time, a.keys[k].value);
        for (k = 0; k < a.keys.length; k++) {
            key = a.keys[k]; mode = key.hold ? KeyframeInterpolationType.HOLD : KeyframeInterpolationType.LINEAR;
            p.setInterpolationTypeAtKey(k + 1, mode, mode);
            if (p.isSpatial) {
                try { p.setSpatialAutoBezierAtKey(k + 1, false); p.setSpatialContinuousAtKey(k + 1, false); p.setSpatialTangentsAtKey(k + 1, [0, 0], [0, 0]); }
                catch (e) { warn("Spatial linearization unavailable: " + e.toString()); }
            }
        }
    }
    function applyLayer(l, b) {
        // Setting start/stretch can move existing keys, so do this before adding comp-time keys.
        l.stretch = b.stretch; l.startTime = b.startTime; l.inPoint = b.inPoint; l.outPoint = b.outPoint; l.enabled = b.enabled;
        try { l.audioEnabled = false; } catch (_) {}
        var t = l.property("ADBE Transform Group"), names = ["anchor", "position", "scale", "rotation", "opacity"], matches = ["ADBE Anchor Point", "ADBE Position", "ADBE Scale", "ADBE Rotate Z", "ADBE Opacity"], n;
        for (n = 0; n < names.length; n++) {
            try { applyProperty(t.property(matches[n]), b.transform[names[n]]); }
            catch (e) { warn(b.name + " " + names[n] + ": " + e.toString()); }
        }
    }
    function footage(path) {
        var f = new File(path);
        if (!f.exists) { warn("Missing footage skipped; relink on desktop: " + path); return null; }
        try { return app.project.importFile(new ImportOptions(f)); }
        catch (e) { warn("Footage import failed " + path + ": " + e.toString()); return null; }
    }
    try {
        app.beginUndoGroup("Import Unwritten interchange"); undoOpen = true;
        for (i = 0; i < data.comps.length; i++) {
            c = data.comps[i]; dest = app.project.items.addComp(c.name, c.width, c.height, c.pixelAspect, c.duration, c.fps);
            dest.bgColor = c.background; ids["comp:" + c.id] = dest; made.push(dest);
        }
        for (i = 0; i < data.comps.length; i++) {
            c = data.comps[i]; dest = ids["comp:" + c.id];
            // AE add inserts at the top. Reverse iteration preserves the original top-to-bottom order.
            for (j = c.layers.length - 1; j >= 0; j--) {
                d = c.layers[j]; nl = null;
                try {
                    if (d.kind === "solid") nl = dest.layers.addSolid(d.color, d.name, d.width, d.height, 1);
                    else if (d.kind === "null") nl = dest.layers.addNull();
                    else if (d.kind === "text") {
                        nl = dest.layers.addText(d.text); st = nl.property("ADBE Text Properties").property("ADBE Text Document"); doc = st.value;
                        if (d.font) { try { doc.font = d.font; } catch (e) { warn(d.name + ": font unavailable: " + d.font); } }
                        doc.fontSize = d.fontSize; doc.applyFill = true; doc.fillColor = d.color; st.setValue(doc);
                    } else if (d.kind === "comp") {
                        src = ids["comp:" + d.source]; if (!src) { warn("Missing precomp skipped: " + d.source); continue; } nl = dest.layers.add(src);
                    } else if (d.kind === "footage") { src = footage(d.source); if (!src) continue; nl = dest.layers.add(src); }
                    else { warn("Unsupported layer skipped: " + d.name); continue; }
                    nl.name = d.name; applyLayer(nl, d);
                } catch (e) { warn("Layer " + d.name + " could not be fully reconstructed: " + e.toString()); }
            }
        }
        app.endUndoGroup(); undoOpen = false;
    } catch (e) {
        if (undoOpen) { try { app.endUndoGroup(); } catch (_) {} }
        alert("Import stopped: " + e.toString() + ". Existing content was not replaced. Partial additions may remain; use Undo to remove this import."); return;
    }
    alert("Reconstructed " + made.length + " composition(s). Warnings: " + warnings.length + summary());
    if (confirm("Save the CURRENT project (including existing content and imported compositions) as a new .aep?\nCancel keeps the imported compositions unsaved.")) {
        saveTo = File.saveDialog("Save reconstructed project under a new AEP name", "After Effects:*.aep");
        if (saveTo) {
            if (!/\.aep$/i.test(saveTo.name)) saveTo = new File(saveTo.fsName + ".aep");
            if (app.project.file && app.project.file.fsName.toLowerCase() === saveTo.fsName.toLowerCase()) { alert("The source AEP may not be overwritten. Use File > Save As with a new name."); return; }
            if (saveTo.exists && !confirm("Replace this existing destination?\n" + saveTo.fsName)) return;
            try { app.project.save(saveTo); saved = true; } catch (e) { alert("AEP save failed: " + e.toString() + ". Imported compositions remain open; use Save As."); return; }
        }
    }
    alert(saved ? "Saved reconstructed AEP: " + saveTo.fsName : "Imported compositions remain open. Choose File > Save As to keep them. No AEP was written.");
}());
