/* Original clean-room Unwritten Studio bridge. Run using AE desktop File > Scripts > Run Script File.
 * Uses only public scripting behaviour; ES3, no JSON global or proprietary AEP decoder.
 */
(function () {
    if (typeof app === "undefined" || !app.project) { alert("Open an After Effects project first."); return; }
    function sameFile(a, b) { return a && b && String(a.fsName).toLowerCase() === String(b.fsName).toLowerCase(); }
    function sourceProject() {
        var current = app.project, input;
        if (confirm("Export the CURRENT After Effects project?\nChoose Cancel to select an existing native .aep or XML .aepx via After Effects.\nThe bridge does not decode project bytes.")) return current;
        if (current.dirty === true || current.modified === true) {
            alert("The current After Effects project has unsaved changes. Save it first, then run the bridge again; the bridge will not close or discard it.");
            return null;
        }
        input = File.openDialog("Choose an After Effects source project (.aep or XML .aepx)", "After Effects projects:*.aep,*.aepx");
        if (!input) return null;
        if (!/\.(aep|aepx)$/i.test(input.name)) { alert("Choose a native .aep or XML .aepx project. The bridge uses public app.open and does not decode project bytes."); return null; }
        if (current.file && sameFile(current.file, input)) return current;
        try {
            var opened = app.open(input);
            if (!opened && app.project === current) { alert("After Effects cancelled opening the selected source; the current project was left untouched."); return null; }
        } catch (e) { alert("Could not open the selected AEP/AEPX source; the current project was left untouched: " + e.toString()); return null; }
        if (!app.project || app.project === current) { alert("After Effects did not switch to the selected source; the current project was left untouched."); return null; }
        return app.project;
    }
    var warnings = ["Experimental 2D bridge: audio, colour management, plugins, markers and unsupported features are not transferred. Keep your original AEP."], comps = [], sources = [];
    function warn(s) { if (warnings.length < 999) warnings.push(String(s)); else if (warnings.length === 999) warnings.push("Further warnings omitted; unsupported features may be present in other layers."); }
    function esc(s) {
        s = String(s); var r = '"', i, n, c;
        for (i = 0; i < s.length; i++) {
            c = s.charAt(i); n = s.charCodeAt(i);
            if (c === "\\") r += "\\\\"; else if (c === '"') r += '\\"';
            else if (n < 32 || n === 8232 || n === 8233) r += "\\u" + ("0000" + n.toString(16)).slice(-4);
            else r += c;
        }
        return r + '"';
    }
    function json(v) {
        var a = [], k, i;
        if (v === null) return "null";
        if (typeof v === "string") return esc(v);
        if (typeof v === "boolean") return v ? "true" : "false";
        if (typeof v === "number") { if (!isFinite(v)) throw new Error("Non-finite numeric value"); return String(v); }
        if (v instanceof Array) { for (i = 0; i < v.length; i++) a.push(json(v[i])); return "[" + a.join(",") + "]"; }
        for (k in v) if (Object.prototype.hasOwnProperty.call(v, k)) a.push(esc(k) + ":" + json(v[k]));
        return "{" + a.join(",") + "}";
    }
    function rgb(v) { return [v[0], v[1], v[2]]; }
    function val(v) { return v instanceof Array ? [v[0], v[1]] : v; }
    function nonzero(v) { var i; for (i = 0; v && i < v.length; i++) if (Math.abs(v[i]) > 0.000001) return true; return false; }
    function anim(p, label) {
        var keys = [], i, hold, base;
        if (!p) throw new Error("Missing " + label);
        if (p.expressionEnabled) warn(label + ": expression omitted; original base values and keyframes used.");
        if (p.dimensionsSeparated) throw new Error(label + ": separated dimensions unsupported");
        try { base = p.valueAtTime(0, true); } catch (_) { base = p.value; }
        for (i = 1; i <= p.numKeys; i++) {
            hold = p.keyOutInterpolationType(i) === KeyframeInterpolationType.HOLD;
            if (p.keyInInterpolationType(i) === KeyframeInterpolationType.BEZIER || p.keyOutInterpolationType(i) === KeyframeInterpolationType.BEZIER) warn(label + ": Bezier temporal easing approximated as linear.");
            if (p.isSpatial && (nonzero(p.keyInSpatialTangent(i)) || nonzero(p.keyOutSpatialTangent(i)))) warn(label + ": spatial Bezier curve approximated as linear.");
            keys.push({ time: p.keyTime(i), value: val(p.keyValue(i)), hold: hold });
        }
        return { value: val(base), keys: keys };
    }
    function transform(l) {
        var t = l.property("ADBE Transform Group"), label = l.name + " ";
        return { anchor: anim(t.property("ADBE Anchor Point"), label + "anchor"), position: anim(t.property("ADBE Position"), label + "position"),
            scale: anim(t.property("ADBE Scale"), label + "scale"), rotation: anim(t.property("ADBE Rotate Z"), label + "rotation"), opacity: anim(t.property("ADBE Opacity"), label + "opacity") };
    }
    function compId(c) { var i; for (i = 0; i < sources.length; i++) if (sources[i] === c) return "comp-" + i; throw new Error("Precomp source unavailable"); }
    function layer(l, c) {
        var s = l.source, doc, st, masks, effects, d;
        if (l.parent || l.threeDLayer || l.adjustmentLayer || l.timeRemapEnabled || l.stretch <= 0 ||
            (typeof l.trackMatteType !== "undefined" && l.trackMatteType !== TrackMatteType.NO_TRACK_MATTE) || l.isTrackMatte) {
            warn("Omitted " + l.name + ": parenting, 3D, matte, adjustment, reverse stretch or time remapping unsupported."); return null;
        }
        if (s instanceof FootageItem && (l.hasVideo === false || s.hasVideo === false)) {
            warn("Omitted " + l.name + ": audio-only footage (layer/source hasVideo=false)."); return null;
        }
        d = { name: String(l.name), kind: "", source: "", width: 100, height: 100, color: [1, 1, 1], text: "", font: "", fontSize: 48,
            startTime: l.startTime, inPoint: l.inPoint, outPoint: l.outPoint, stretch: l.stretch, enabled: l.enabled !== false, transform: transform(l) };
        if (l instanceof TextLayer) {
            st = l.property("ADBE Text Properties").property("ADBE Text Document"); doc = st.value;
            d.kind = "text"; d.text = String(doc.text); d.font = String(doc.font); d.fontSize = doc.fontSize;
            if (doc.applyFill) d.color = rgb(doc.fillColor);
            warn(l.name + ": text transfers current text and base font/size/fill only; text animation, paragraph settings, strokes and mixed styling omitted.");
        } else if (l.nullLayer) d.kind = "null";
        else if (s instanceof CompItem) { d.kind = "comp"; d.source = compId(s); d.width = s.width; d.height = s.height; }
        else if (s instanceof FootageItem) {
            d.width = s.width; d.height = s.height;
            if (s.mainSource instanceof SolidSource) { d.kind = "solid"; d.color = rgb(s.mainSource.color); }
            else if (s.file) {
                if (!s.mainSource.isStill && /\.(png|jpe?g|tiff?|exr|bmp)$/i.test(s.file.name)) { warn("Omitted " + l.name + ": image sequences unsupported."); return null; }
                d.kind = "footage"; d.source = s.file.fsName;
            } else { warn("Omitted " + l.name + ": missing footage source."); return null; }
        } else { warn("Omitted " + l.name + ": unsupported layer type."); return null; }
        masks = l.property("ADBE Mask Parade"); effects = l.property("ADBE Effect Parade");
        if (masks && masks.numProperties) warn(l.name + ": masks omitted.");
        if (effects && effects.numProperties) warn(l.name + ": effects omitted.");
        if (typeof BlendingMode !== "undefined" && l.blendingMode !== BlendingMode.NORMAL) warn(l.name + ": blend mode approximated as Normal.");
        return d;
    }
    var i, j, c, layers, d, sourceProjectValue;
    try {
        sourceProjectValue = sourceProject();
        if (!sourceProjectValue) return;
        for (i = 1; i <= sourceProjectValue.numItems; i++) if (sourceProjectValue.item(i) instanceof CompItem) sources.push(sourceProjectValue.item(i));
        if (!sources.length) { alert("No composition to export."); return; }
        for (i = 0; i < sources.length; i++) {
            c = sources[i]; layers = [];
            for (j = 1; j <= c.numLayers; j++) {
                try { d = layer(c.layer(j), c); if (d) layers.push(d); } catch (e) { warn("Omitted " + c.layer(j).name + ": " + e.toString()); }
            }
            comps.push({ id: "comp-" + i, name: String(c.name), width: c.width, height: c.height, fps: c.frameRate, duration: c.duration,
                pixelAspect: c.pixelAspect, background: rgb(c.bgColor), layers: layers });
        }
        var text = json({ format: "unwritten-after-effects", version: 1, warnings: warnings, comps: comps });
        if (text.length > 16000000) { alert("Bridge too large; export fewer compositions (32 MiB UTF-8 import limit)."); return; }
        var file = File.saveDialog("Export Unwritten interchange (.unwae)", "Unwritten AE:*.unwae");
        if (!file) return;
        if (!/\.unwae$/i.test(file.name)) file = new File(file.fsName + ".unwae");
        if (file.exists && !confirm("Replace existing interchange file?\n" + file.fsName)) return;
        file.encoding = "UTF-8";
        if (!file.open("w")) throw new Error("Cannot open destination: " + file.error);
        try { if (!file.write(text)) throw new Error("Cannot write destination: " + file.error); } finally { file.close(); }
        alert("Exported " + comps.length + " composition(s) to " + file.fsName + ".\nWarnings: " + warnings.length + ". See the warnings array in .unwae. The original AEP was not modified.");
    } catch (e) { alert("Interchange export failed: " + e.toString()); }
}());
