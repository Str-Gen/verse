const $ = id => document.getElementById(id);
function element(tag, text) { const node = document.createElement(tag); if (text !== undefined) node.textContent = text; return node; }
function button(text, action) { const node = element('button', text); node.onclick = action; return node; }
export function definitionText(result) {
  if (!result.available) return 'The local dictionary is not installed. Set VERSE_DICTIONARY to its SQLite file.';
  if (!result.entries.length) return 'This word is not in the local dictionary.';
  return result.entries.map(entry => [entry.word + ' · ' + (entry.pos ?? ''), entry.etymology_text,
    ...entry.senses.map(sense => [...(sense.tags ?? []), ...(sense.raw_tags ?? []), ...(sense.glosses ?? [])].join(' · ')),
    'https://en.wiktionary.org/wiki/' + encodeURIComponent(entry.word)].filter(Boolean).join('\n')).join('\n\n') + '\n\nWiktionary contributors · Kaikki / Wiktextract · CC BY-SA 4.0\nhttps://creativecommons.org/licenses/by-sa/4.0/';
}
export class VocabularyUI {
  constructor(request, pause, speed, notice) {
    Object.assign(this, {request, pause, speed, notice}); this.words = []; this.generation = 0;
    $('capture-button').onclick = () => this.capture();
    $('vocabulary-button').onclick = () => this.open();
    $('vocabulary-close').onclick = () => $('vocabulary').close();
    $('vocabulary').onclose = () => clearTimeout(this.poll);
    $('practice-start').onclick = () => this.practice();
    $('practice-close').onclick = () => { clearTimeout(this.flash); $('practice').close(); $('vocabulary').showModal(); this.refresh(); };
    $('practice-reveal').onclick = () => this.reveal();
    for (const choice of ['comfortable', 'difficult', 'skip']) $('practice-' + choice).onclick = () => { this.counts[choice]++; this.practiceAt++; this.card(); };
  }
  reset() { this.generation++; clearTimeout(this.poll); clearTimeout(this.flash); this.words = []; }
  async capture() {
    try { await this.request({action:'capture', scope:$('capture-scope').value}); this.notice('Captured. Your book’s word list is stored for later review · V.'); }
    catch (error) { this.notice(String(error)); }
  }
  open() { this.pause(); $('vocabulary').showModal(); this.refresh(); }
  async refresh() {
    const generation = this.generation;
    try {
      const result = await this.request({action:'review'});
      if (generation !== this.generation) return;
      this.words = result.words;
      const rows = $('vocabulary-rows'); rows.replaceChildren();
      $('vocabulary-status').textContent = result.captures.length ? `${result.captures.length} captures · ${result.words.length} saved words` : 'Press C during playback to capture recent words.';
      for (const capture of result.captures.filter(c => c.status !== 'ranked')) {
        const row = element('p', capture.error || 'Ranking captured words…');
        if (capture.status !== 'ranking') row.append(button('Retry ranking', async () => { try { await this.request({action:'retry', capture:capture.id}); this.refresh(); } catch(e) { this.notice(String(e)); } }));
        rows.append(row);
      }
      for (const suggestion of result.suggestions) {
        const candidate = suggestion.candidate, row = element('section'), context = element('p');
        if (candidate.sentence) {
          const chars = Array.from(candidate.sentence);
          context.append(document.createTextNode(chars.slice(0,candidate.sentence_start).join('')), element('strong', chars.slice(candidate.sentence_start,candidate.sentence_end).join('')), document.createTextNode(chars.slice(candidate.sentence_end).join('')));
        } else context.textContent = `${candidate.word} · Captured partial context: ${candidate.context}`;
        const choices = element('div'), definition = element('pre'); definition.hidden = true;
        for (const [choice, label] of [['unfamiliar','A · Unfamiliar'],['recognition','B · Know it but slow to recognize'],['dismissed','C · Dismiss']]) {
          const control = button(label, async () => { try { await this.request({action:'feedback', capture:suggestion.capture, candidate:suggestion.candidate_index, choice}); this.refresh(); } catch(e) { this.notice(String(e)); } });
          control.setAttribute('aria-pressed', String(suggestion.choice === choice)); choices.append(control);
        }
        row.append(context, choices, button('See definition / collapse', async () => {
          definition.hidden = !definition.hidden; choices.hidden = !definition.hidden;
          if (!definition.hidden) { clearTimeout(this.poll); try { definition.textContent = definitionText(await this.request({action:'definition', word:candidate.word})); } catch(e) { definition.textContent = String(e); } }
          else this.refresh();
        }), definition); rows.append(row);
      }
      if ($('vocabulary').open && result.captures.some(c => c.status === 'ranking')) this.poll = setTimeout(() => this.refresh(), 1000);
    } catch(error) { $('vocabulary-status').textContent = String(error); }
  }
  practice() {
    if (!this.words.length) { $('vocabulary-status').textContent = 'Save a word as unfamiliar or slow to recognize first.'; return; }
    clearTimeout(this.poll); $('vocabulary').close(); $('practice').showModal();
    this.practiceAt = 0; this.counts = {comfortable:0, difficult:0, skip:0}; this.card();
  }
  card() {
    clearTimeout(this.flash); $('practice-context').textContent = ''; $('practice-definition').textContent = '';
    $('practice-counts').textContent = `Comfortable ${this.counts.comfortable} · Difficult ${this.counts.difficult} · Skipped ${this.counts.skip}`;
    const word = this.words[this.practiceAt]; $('practice-word').textContent = word?.word ?? 'Session complete';
    for (const id of ['reveal','comfortable','difficult','skip']) $('practice-' + id).disabled = !word;
    if (word) this.flash = setTimeout(() => { $('practice-word').textContent = '•••'; }, 60000 / this.speed());
  }
  async reveal() {
    const at = this.practiceAt, word = this.words[at]; if (!word) return;
    $('practice-context').textContent = word.candidate.sentence ?? 'Captured partial context: ' + word.candidate.context;
    try { const result = await this.request({action:'definition', word:word.word}); if (at === this.practiceAt) $('practice-definition').textContent = definitionText(result); }
    catch(error) { $('practice-definition').textContent = String(error); }
  }
}
