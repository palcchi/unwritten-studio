// Original mock-DOM smoke tests, not an Adobe fidelity certification.
// Run: node scripts/test-ae-bridge.cjs
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '..');
const exporter = fs.readFileSync(path.join(root, 'crates/engine/scripts/Export Unwritten AE Bridge.jsx'), 'utf8');
const importer = fs.readFileSync(path.join(root, 'crates/engine/scripts/Import Unwritten AE Bridge.jsx'), 'utf8');
assert.equal(importer.split('__UNWRITTEN_PAYLOAD__').length, 2, 'one substitution site');
const mocks = `
var JSON = undefined, alerts = [], output = '', written = false, saved = null, replaceAccepted = true;
var KeyframeInterpolationType = {HOLD:'hold',LINEAR:'linear',BEZIER:'bezier'};
var TrackMatteType = {NO_TRACK_MATTE:0}, BlendingMode = {NORMAL:0};
function Prop(value, keys) {
  this.value = value; this.keys = keys || []; this.numKeys = this.keys.length; this.modes = [];
  this.isSpatial = value instanceof Array; this.expressionEnabled = false;
  this.valueAtTime = function(){return this.value;};
  this.keyValue = function(i){return this.keys[i-1].value;};
  this.keyTime = function(i){return this.keys[i-1].time;};
  this.keyInInterpolationType = this.keyOutInterpolationType = function(i){return this.keys[i-1].hold?'hold':'linear';};
  this.keyInSpatialTangent = this.keyOutSpatialTangent = function(){return [0,0];};
  this.setValue = function(v){this.value=v;};
  this.setValueAtTime = function(t,v){this.keys.push({time:t,value:v});};
  this.setInterpolationTypeAtKey = function(i,a,b){this.modes[i-1]=[a,b];};
  this.setSpatialAutoBezierAtKey = this.setSpatialContinuousAtKey = this.setSpatialTangentsAtKey = function(){};
}
function TextLayer(){}
function SolidSource(){this.color=[0.1,0.2,0.3];this.isStill=true;}
function FootageItem(){this.width=100;this.height=80;this.hasVideo=true;this.mainSource=new SolidSource();}
function Layer(name,kind) {
  this.name=name;this.source=kind==='null'?null:new FootageItem();this.nullLayer=kind==='null';
  this.parent=null;this.threeDLayer=false;this.adjustmentLayer=false;this.timeRemapEnabled=false;
  this.trackMatteType=0;this.isTrackMatte=false;this.blendingMode=0;this.enabled=true;
  this.startTime=1;this.inPoint=1;this.outPoint=3;this.stretch=200;
  this.props={'ADBE Anchor Point':new Prop([0,0]),'ADBE Position':new Prop([10,20],[{time:1,value:[10,20],hold:true},{time:2,value:[30,40]}]),
    'ADBE Scale':new Prop([100,100]),'ADBE Rotate Z':new Prop(15),'ADBE Opacity':new Prop(100)};
  var self=this;
  this.property=function(n){if(n==='ADBE Transform Group')return {property:function(x){return self.props[x];}};return {numProperties:0};};
}
function CompItem(name){
  this.name=name;this.width=320;this.height=180;this.pixelAspect=1;this.duration=3;this.frameRate=24;this.bgColor=[0,0,0];
  this.arr=[];this.numLayers=0;var self=this;this.layer=function(i){return self.arr[i-1];};
  this.layers={addSolid:function(c,n,w,h){var l=new Layer(n,'solid');self.arr.unshift(l);return l;},
    addNull:function(){var l=new Layer('null','null');self.arr.unshift(l);return l;},
    add:function(src){var l=new Layer(src.name,'solid');l.source=src;self.arr.unshift(l);return l;},
    addText:function(text){var l=new Layer(text,'text'),doc={text:text,font:'Arial',fontSize:48,fillColor:[1,1,1]};
      l.property0=l.property;l.property=function(n){if(n==='ADBE Text Properties')return {property:function(){return {value:doc,setValue:function(v){doc=v;}};}};return l.property0(n);};self.arr.unshift(l);return l;}};
}
function File(p){if(!(this instanceof File))return new File(p);this.fsName=p;this.name=p.split('/').pop();this.exists=false;this.encoding='';}
File.prototype.open=function(){return true;};File.prototype.write=function(s){output=s;written=true;return true;};File.prototype.close=function(){};
File.saveDialog=function(){return new File('/tmp/result.unwae');};
File.openDialog=function(){return null;};
function ImportOptions(f){this.file=f;}
function alert(s){alerts.push(s);}
function confirm(s){if(s.indexOf('Replace')>=0)return replaceAccepted;return true;}
var source = new CompItem('日本語 \\"Cue\\" \\u2028'), nested = new CompItem('Nested'), solid = new Layer('Top','solid'), nullLayer = new Layer('Bottom','null');
var precomp = new Layer('Precomp','solid');precomp.source=nested;source.arr=[solid,nullLayer,precomp];source.numLayers=3;
var app={project:{numItems:2,item:function(i){return i===1?source:nested;},file:null},beginUndoGroup:function(){},endUndoGroup:function(){}};
`;
function context() { const ctx = vm.createContext({}); vm.runInContext(mocks, ctx); return ctx; }
const e = context();
vm.runInContext(exporter, e);
assert.equal(e.written, true, e.alerts.join('\n'));
const data = JSON.parse(e.output);
assert.equal(data.comps.length, 2);
assert.deepEqual(data.comps[0].layers.map(l => l.kind), ['solid', 'null', 'comp']);
assert.equal(data.comps[0].layers[0].fontSize, 48);
assert.equal(data.comps[0].layers[0].transform.position.keys[0].hold, true);
assert.match(data.comps[0].name, /日本語/);
assert.ok(!e.output.includes('\u2028'), 'line separator escaped');
const audioOnly = context();
vm.runInContext("var audio=new Layer('Audio Only','solid');audio.source=new FootageItem();audio.source.hasVideo=false;audio.hasVideo=false;source.arr.push(audio);source.numLayers=4;", audioOnly);
vm.runInContext(exporter, audioOnly);
const audioData = JSON.parse(audioOnly.output);
assert.equal(audioData.comps[0].layers.some(l => l.name === 'Audio Only'), false, 'audio-only footage is not exported');
assert.match(audioData.warnings.join('\n'), /Audio Only.*audio-only footage.*hasVideo=false/, 'audio omission names the exact layer');
const cancelled = context();
vm.runInContext("replaceAccepted=false;File.saveDialog=function(){var f=new File('/tmp/existing.unwae');f.exists=true;return f;};", cancelled);
vm.runInContext(exporter, cancelled);
assert.equal(cancelled.written, false, 'existing interchange not overwritten without confirmation');
const selected = context();
vm.runInContext(`var selectedProject={numItems:2,item:function(i){return i===1?source:nested;},file:new File('/tmp/source.aep'),dirty:false};
confirm=function(s){return s.indexOf('CURRENT')<0;}; File.openDialog=function(){return new File('/tmp/source.aep');};
app.open=function(f){app.project=selectedProject;};`, selected);
vm.runInContext(exporter, selected);
assert.equal(selected.written, true, 'selected AEP is exported');
assert.equal(selected.app.project.file.fsName, '/tmp/source.aep');
const selectedXml = context();
vm.runInContext(`var selectedProject={numItems:2,item:function(i){return i===1?source:nested;},file:new File('/tmp/source.aepx'),dirty:false};
confirm=function(s){return s.indexOf('CURRENT')<0;}; File.openDialog=function(){return new File('/tmp/source.aepx');};
app.open=function(f){openedPath=f.name; app.project=selectedProject;};`, selectedXml);
vm.runInContext(exporter, selectedXml);
assert.equal(selectedXml.openedPath, 'source.aepx');
assert.equal(selectedXml.written, true, 'XML AEPX is opened through public app.open');
const openCancelled = context();
vm.runInContext(`confirm=function(s){return s.indexOf('CURRENT')<0;}; File.openDialog=function(){return new File('/tmp/cancel.aepx');};
app.open=function(){return null;};`, openCancelled);
vm.runInContext(exporter, openCancelled);
assert.equal(openCancelled.written, false, 'cancelled source open leaves current project untouched');
const unsaved = context();
vm.runInContext(`app.project.dirty=true; confirm=function(s){return s.indexOf('CURRENT')<0;};
File.openDialog=function(){throw new Error('must not open before unsaved guard');};`, unsaved);
vm.runInContext(exporter, unsaved);
assert.equal(unsaved.written, false, 'unsaved current project is not discarded');
assert.match(unsaved.alerts.join('\n'), /unsaved changes/);
const dest = context();
vm.runInContext(`var created=[];app.project.items={addComp:function(n){var c=new CompItem(n);created.push(c);return c;}};
app.project.save=function(f){saved=f.fsName;};File.saveDialog=function(){return new File('/tmp/new-project.aep');};`, dest);
vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', JSON.stringify(data)), dest);
assert.equal(dest.created.length, 2);
const main = dest.created[0];
assert.deepEqual(Array.from(main.arr, x => x.name), ['Top', 'Bottom', 'Precomp']);
const p = main.arr[0].props['ADBE Position'];
assert.equal(p.keys[0].time, 1);
assert.equal(p.modes[0][0], 'hold', 'first key interpolation preserved');
assert.equal(p.modes[1][0], 'linear');
assert.equal(dest.saved, '/tmp/new-project.aep', 'uses actual public project.save(file) API');
const noReplace = context();
vm.runInContext(`var created=[];app.project.items={addComp:function(n){var c=new CompItem(n);created.push(c);return c;}};app.project.file=new File('/tmp/source.aep');
app.project.save=function(f){saved=f.fsName;};File.saveDialog=function(){return new File('/tmp/source.aep');};`, noReplace);
vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', JSON.stringify(data)), noReplace);
assert.equal(noReplace.saved, null, 'source AEP must not be overwritten');
const rejectedReplace = context();
vm.runInContext(`var created=[];app.project.items={addComp:function(n){var c=new CompItem(n);created.push(c);return c;}};
app.project.file=new File('/tmp/source.aep'); app.project.save=function(f){saved=f.fsName;};
File.saveDialog=function(){var f=new File('/tmp/existing.aep');f.exists=true;return f;};
replaceAccepted=false;`, rejectedReplace);
vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', JSON.stringify(data)), rejectedReplace);
assert.equal(rejectedReplace.saved, null, 'existing AEP replacement is rejected safely');
const saveFailure = context();
vm.runInContext(`var created=[];app.project.items={addComp:function(n){var c=new CompItem(n);created.push(c);return c;}};
app.project.save=function(){throw new Error('disk full');}; File.saveDialog=function(){return new File('/tmp/failure.aep');};`, saveFailure);
vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', JSON.stringify(data)), saveFailure);
assert.equal(saveFailure.created.length, 2, 'save failure keeps imported compositions open');
assert.match(saveFailure.alerts.join('\n'), /AEP save failed/);
const saveCancelled = context();
vm.runInContext(`var created=[];app.project.items={addComp:function(n){var c=new CompItem(n);created.push(c);return c;}};
app.project.save=function(){saved='unexpected';}; File.saveDialog=function(){return null;};`, saveCancelled);
vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', JSON.stringify(data)), saveCancelled);
assert.equal(saveCancelled.saved, null, 'cancelled save does not write an AEP');
const malformed = context();
vm.runInContext(`var created=[];app.project.items={addComp:function(n){created.push(n);return new CompItem(n);}};`, malformed);
vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', 'null'), malformed);
assert.equal(malformed.created.length, 0, 'malformed interchange must not create compositions');
assert.match(malformed.alerts.join('\n'), /Invalid or unsupported/);
const invalidEnvelope = JSON.parse(JSON.stringify(data));
invalidEnvelope.comps[0].width = 0;
const invalidDimensions = context();
vm.runInContext(`var created=[];app.project.items={addComp:function(n){created.push(n);return new CompItem(n);}};`, invalidDimensions);
vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', JSON.stringify(invalidEnvelope)), invalidDimensions);
assert.equal(invalidDimensions.created.length, 0, 'out-of-range dimensions must not create compositions');
assert.match(invalidDimensions.alerts.join('\n'), /Invalid or unsupported/);
const invalid3d = JSON.parse(JSON.stringify(data));
invalid3d.comps[0].layers[0].transform.position.value = [10, 20, 30];
const invalidVector = context();
vm.runInContext(`var created=[];app.project.items={addComp:function(n){created.push(n);return new CompItem(n);}};`, invalidVector);
vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', JSON.stringify(invalid3d)), invalidVector);
assert.equal(invalidVector.created.length, 0, '3D vectors must not enter the 2D bridge');
assert.match(invalidVector.alerts.join('\n'), /Invalid or unsupported/);
function rejectsBeforeMutation(mutator, message) {
  const hostile = JSON.parse(JSON.stringify(data)); mutator(hostile);
  const c = context(); vm.runInContext(`var created=[];app.project.items={addComp:function(n){created.push(n);return new CompItem(n);}};`, c);
  vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', JSON.stringify(hostile)), c);
  assert.equal(c.created.length, 0, message); assert.match(c.alerts.join('\n'), /Invalid or unsupported/);
}
rejectsBeforeMutation(v => { v.comps[0].background = [2, 0, 0]; }, 'background bounds are checked before mutation');
rejectsBeforeMutation(v => { v.comps[0].layers[0].color = [0, NaN, 0]; }, 'colour bounds are checked before mutation');
rejectsBeforeMutation(v => { v.comps[0].layers[0].fontSize = 10001; }, 'font size bounds are checked before mutation');
rejectsBeforeMutation(v => { v.comps[0].layers[0].enabled = 'yes'; }, 'boolean enum values are checked before mutation');
rejectsBeforeMutation(v => { v.comps[0].layers[0].transform.position.keys[0].hold = 1; }, 'key enum values are checked before mutation');
rejectsBeforeMutation(v => { v.comps[0].layers[0].source = 'missing'; v.comps[0].layers[0].kind = 'comp'; }, 'missing references are checked before mutation');
 rejectsBeforeMutation(v => { v.comps[0].layers[0].kind = 'comp'; v.comps[0].layers[0].source = 'comp-0'; }, 'cyclic references are checked before mutation');
 rejectsBeforeMutation(v => { v.comps[0].layers[0].transform.position.keys[0].time = 2; v.comps[0].layers[0].transform.position.keys[1].time = 1; }, 'key order is checked before mutation');
const duplicateId = JSON.parse(JSON.stringify(data));
duplicateId.comps[1].id = duplicateId.comps[0].id;
const duplicateContext = context();
vm.runInContext(`var created=[];app.project.items={addComp:function(n){created.push(n);return new CompItem(n);}};`, duplicateContext);
vm.runInContext(importer.replace('__UNWRITTEN_PAYLOAD__', JSON.stringify(duplicateId)), duplicateContext);
assert.equal(duplicateContext.created.length, 0, 'duplicate composition IDs are rejected before mutation');
assert.match(duplicateContext.alerts.join('\n'), /Invalid or unsupported/);
console.log('AE bridge: exporter/importer VM smoke tests passed (mock AE DOM, no fidelity claim).');
