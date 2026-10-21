const systemDark = matchMedia('(prefers-color-scheme: dark)');
let palette = {};
export function applyTheme(values) {
  palette = values;
  const background = values.background;
  const rgb = background?.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  const dark = values.mode ? values.mode === 'dark' : rgb ?
    .299 * parseInt(rgb[1], 16) + .587 * parseInt(rgb[2], 16) + .114 * parseInt(rgb[3], 16) < 127.5 : systemDark.matches;
  const ink = values.foreground || (dark ? '#eeeeee' : '#222324');
  const selection = values.selection || values.selection_background || (dark ? '#186a9a' : '#2077b2');
  const styles = {
    '--paper': background || (dark ? '#101010' : '#ffffff'),
    '--ink': ink,
    '--accent': values.accent || (dark ? '#5584aa' : '#2077b2'),
    '--selection': selection,
    '--selected-text': values.selection_foreground || values.bright_foreground || ink,
    '--muted': ink,
    '--line': values.muted || selection,
  };
  for (const [key, value] of Object.entries(styles)) document.documentElement.style.setProperty(key, value);
  document.documentElement.style.colorScheme = dark ? 'dark' : 'light';
}
systemDark.addEventListener('change', () => applyTheme(palette));
export async function startTheme(native) {
  if (!native) { applyTheme({}); return; }
  async function refresh() {
    try { applyTheme(await native.invoke('read_theme')); }
    catch (error) { console.warn('Could not refresh reader theme', error); }
  }
  await refresh();
  // Reopen the path each time: theme switches may replace files or symlinks.
  async function poll() { await refresh(); setTimeout(poll, 1000); }
  setTimeout(poll, 1000);
}
