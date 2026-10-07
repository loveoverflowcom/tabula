// Tool-only lowering of Rust's validated RenderList. I-5/I-10; no game rules.
export class BoundaryError extends Error {
  constructor(code, path) { super(`${code}: ${path}`); this.name = 'BoundaryError'; this.code = code; this.path = path; }
}
export function requireThat(value, code, path) { if (!value) throw new BoundaryError(code, path); }
export function record(value, path) {
  requireThat(value && typeof value === 'object' && !Array.isArray(value) &&
    (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null), 'invalid_record', path);
  return value;
}
export function exact(value, fields, path) {
  record(value, path);
  const keys = Object.keys(value);
  requireThat(keys.length === fields.length && fields.every(key => Object.hasOwn(value, key)) && keys.every(key => fields.includes(key)), 'unknown_or_missing_field', path);
  return value;
}
export function finite(value, path, min = -1e7, max = 1e7) {
  requireThat(typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max, 'invalid_number', path);
  return value;
}
export function uint(value, path, max = Number.MAX_SAFE_INTEGER) {
  requireThat(Number.isSafeInteger(value) && value >= 0 && value <= max, 'invalid_integer', path); return value;
}
export function string(value, path, max = 4096) { requireThat(typeof value === 'string' && value.length <= max, 'invalid_string', path); return value; }
export function boolean(value, path) { requireThat(typeof value === 'boolean', 'invalid_boolean', path); return value; }
export function tuple(value, n, path, min = -1e7, max = 1e7) {
  requireThat(Array.isArray(value) && value.length === n, 'invalid_tuple', path);
  value.forEach((v, i) => finite(v, `${path}[${i}]`, min, max)); return value;
}
const color = (v, p) => {tuple(v, 4, p, 0, 255);requireThat(v.every(Number.isInteger),'invalid_color',p);};
const rect = (v, p) => { tuple(v, 4, p); finite(v[2], `${p}.width`, 0); finite(v[3], `${p}.height`, 0); };
const border = (v, p) => { exact(v, ['width', 'color'], p); finite(v.width, `${p}.width`, 0, 10000); color(v.color, `${p}.color`); };
function paint(value, path) {
  if (value === null) return;
  record(value, path);
  if (value.kind === 'solid') { exact(value, ['kind', 'color'], path); color(value.color, `${path}.color`); }
  else if (value.kind === 'linear_gradient') {
    exact(value, ['kind', 'from', 'to', 'stops'], path); tuple(value.from, 2, `${path}.from`); tuple(value.to, 2, `${path}.to`);
    requireThat(Array.isArray(value.stops) && value.stops.length >= 2 && value.stops.length <= 64, 'invalid_gradient', path);
    let offset = -1;
    value.stops.forEach((stop, i) => { const p = `${path}.stops[${i}]`; exact(stop, ['offset', 'color'], p); finite(stop.offset, `${p}.offset`, 0, 1); requireThat(stop.offset >= offset, 'unsorted_gradient', p); offset = stop.offset; color(stop.color, `${p}.color`); });
  } else throw new BoundaryError('unsupported_paint', path);
}
export function validateViewport(value) { exact(value, ['width', 'height', 'dpi'], 'viewport'); finite(value.width, 'viewport.width', 1, 8192); finite(value.height, 'viewport.height', 1, 8192); finite(value.dpi, 'viewport.dpi', 0.25, 4); return value; }
export function validatePreferences(value) { exact(value, ['reduced_motion', 'audio_enabled', 'volume', 'modal'], 'preferences'); boolean(value.reduced_motion, 'preferences.reduced_motion'); boolean(value.audio_enabled, 'preferences.audio_enabled'); finite(value.volume, 'preferences.volume', 0, 1); boolean(value.modal, 'preferences.modal'); return value; }
export function validateIdentity(value) { exact(value, ['session_id', 'generation'], 'identity'); string(value.session_id, 'identity.session_id', 128); requireThat(value.session_id.length > 0, 'empty_session', 'identity'); uint(value.generation, 'identity.generation'); return value; }
export function validateInput(value) {
  record(value, 'input');
  switch (value.kind) {
    case 'pointer': exact(value, ['kind', 'position', 'button', 'phase'], 'input'); tuple(value.position, 2, 'input.position'); requireThat(['primary', 'secondary', 'middle'].includes(value.button) && ['down', 'move', 'up', 'cancel'].includes(value.phase), 'invalid_pointer', 'input'); break;
    case 'key': exact(value, ['kind', 'key', 'pressed'], 'input'); requireThat(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Enter', 'Space', 'Escape', 'Tab'].includes(value.key), 'unsupported_key', 'input'); boolean(value.pressed, 'input.pressed'); break;
    case 'focus': exact(value, ['kind', 'focused'], 'input'); boolean(value.focused, 'input.focused'); break;
    default: throw new BoundaryError('unsupported_input', 'input.kind');
  }
  return value;
}
// The required field lists are emitted by Rust, never duplicated as JS/TS wire types.
export function validateFrame(frame, contract, assets = null, themes = null) {
  requireThat(contract?.schema_version === 1 && contract.commands, 'schema_version', 'contract');
  requireThat(Array.isArray(contract.frame_fields),'missing_frame_schema','contract');exact(frame, contract.frame_fields, 'frame');
  finite(frame.at_ms, 'frame.at_ms', 0); string(frame.checkpoint, 'frame.checkpoint', 128); uint(frame.input_count, 'frame.input_count'); uint(frame.board_cells, 'frame.board_cells', 100000);
  uint(frame.script_steps,'frame.script_steps');tuple(frame.viewport,2,'frame.viewport',1,8192);finite(frame.dpi,'frame.dpi',0.25,4);requireThat(contract.themes.includes(frame.theme),'unknown_theme','frame.theme');boolean(frame.reduced_motion,'frame.reduced_motion');
  exact(frame.camera, ['origin', 'zoom'], 'frame.camera'); tuple(frame.camera.origin, 2, 'frame.camera.origin'); finite(frame.camera.zoom, 'frame.camera.zoom', 0.001, 1000); string(frame.a11y, 'frame.a11y');
  requireThat(Array.isArray(frame.commands) && frame.commands.length <= 10000, 'command_limit', 'frame.commands');
  const scopes = [];
  frame.commands.forEach((cmd, i) => {
    const p = `frame.commands[${i}]`; record(cmd, p);
    const fields = contract.commands[cmd.kind]; requireThat(Array.isArray(fields), 'unsupported_command', `${p}.kind`); exact(cmd, fields, p);
    uint(cmd.layer, `${p}.layer`, 255); requireThat(Number.isInteger(cmd.z) && cmd.z >= -32768 && cmd.z <= 32767, 'invalid_z', p);
    switch (cmd.kind) {
      case 'sprite': string(cmd.asset, `${p}.asset`, 256); requireThat(!assets || assets.resources.some(r => r.id === cmd.asset), 'missing_asset', p); rect(cmd.rect, `${p}.rect`); color(cmd.tint, `${p}.tint`); finite(cmd.rotation, `${p}.rotation`); tuple(cmd.pivot, 2, `${p}.pivot`); break;
      case 'rect': rect(cmd.rect, `${p}.rect`); tuple(cmd.radii, 4, `${p}.radii`, 0); paint(cmd.fill, `${p}.fill`); if (cmd.border !== null) border(cmd.border, `${p}.border`); break;
      case 'text': string(cmd.text, `${p}.text`); tuple(cmd.at, 2, `${p}.at`); string(cmd.style, `${p}.style`, 128); requireThat(!themes || Object.values(themes).some(t => Object.hasOwn(t.text_styles, cmd.style)), 'unknown_text_style', p); requireThat(['start', 'center', 'end'].includes(cmd.align), 'invalid_alignment', p); if (cmd.max_width !== null) finite(cmd.max_width, `${p}.max_width`, Number.MIN_VALUE); color(cmd.color, `${p}.color`); break;
      case 'path': requireThat(Array.isArray(cmd.points) && cmd.points.length >= 2 && cmd.points.length <= 1024, 'invalid_path', p); cmd.points.forEach((point, n) => tuple(point, 2, `${p}.points[${n}]`)); border(cmd.stroke, `${p}.stroke`); boolean(cmd.closed, `${p}.closed`); paint(cmd.fill, `${p}.fill`); break;
      case 'push_clip': rect(cmd.rect, `${p}.rect`); scopes.push('clip'); break;
      case 'push_transform': tuple(cmd.matrix, 6, `${p}.matrix`); scopes.push('transform'); break;
      case 'push_opacity': finite(cmd.opacity, `${p}.opacity`, 0, 1); scopes.push('opacity'); break;
      case 'pop_clip': case 'pop_transform': case 'pop_opacity': requireThat(scopes.pop() === cmd.kind.slice(4), 'unbalanced_scope', p); break;
      default: throw new BoundaryError('unsupported_command', p);
    }
    requireThat(scopes.length <= 64, 'scope_limit', p);
  });
  requireThat(scopes.length === 0, 'unbalanced_scope', 'frame.commands');
  return frame;
}
export function validateView(value, identity, lastRevision, contract, assets, themes) {
  exact(value, ['schema_version', 'session_id', 'generation', 'revision', 'frame'], 'view');
  requireThat(value.schema_version === 1, 'schema_version', 'view');
  requireThat(value.session_id === identity.session_id && value.generation === identity.generation, 'stale_identity', 'view');
  uint(value.revision, 'view.revision'); requireThat(value.revision > lastRevision, 'stale_revision', 'view');
  validateFrame(value.frame, contract, assets, themes);
  // Clone at ingress: caller mutation cannot bypass validation after acceptance.
  return structuredClone(value);
}
