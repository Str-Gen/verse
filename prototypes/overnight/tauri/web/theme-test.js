export async function runTests() {
  const checks = [], errors = [];
  for (const [name, paper, ink, accent, selection, selected] of [
    ['initial palette', '#112233', '#eeeeee', '#abcdef', '#334455', '#ffffff'],
    ['atomic replacement', '#fefefe', '#222222', '#123456', '#ccddee', '#111111'],
    ['symlink switch', '#223344', '#dddddd', '#fedcba', '#556677', '#eeeeee'],
    ['missing file fallback', null, null, null, null, null],
    ['recreated theme', '#112233', '#eeeeee', '#abcdef', '#334455', '#ffffff'],
  ]) {
    const deadline = Date.now() + 15000;
    let passed = false;
    while (Date.now() < deadline) {
      const style = document.documentElement.style;
      passed = paper ? [['--paper', paper], ['--ink', ink], ['--accent', accent], ['--selection', selection], ['--selected-text', selected]].every(([key, value]) => style.getPropertyValue(key) === value) : ['#101010', '#ffffff'].includes(style.getPropertyValue('--paper'));
      if (passed && paper) {
        const rgb = hex => `rgb(${[1, 3, 5].map(offset => parseInt(hex.slice(offset, offset + 2), 16)).join(', ')})`;
        const computed = getComputedStyle(document.documentElement);
        passed = computed.backgroundColor === rgb(paper) && computed.color === rgb(ink);
      }
      if (passed) break;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    checks.push({ name, passed });
    if (!passed) errors.push(name);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  return { passed: errors.length === 0, checks, errors };
}
