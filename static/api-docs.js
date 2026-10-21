(() => {
  const root = document.querySelector('[data-api-docs]');
  if (!root) return;
  const tree = root.querySelector('.api-index-disclosure');
  if (tree) {
    const narrow = window.matchMedia('(max-width: 800px)');
    const updateTree = () => { tree.open = !narrow.matches; };
    updateTree();
    narrow.addEventListener('change', updateTree);
  }
  const input = root.querySelector('#api-search');
  const entries = [...root.querySelectorAll('[data-api-entry]')];
  const index = new Map([...root.querySelectorAll('[data-api-index]')].map(item => [item.dataset.apiIndex, item]));
  if (input) {
    root.querySelector('.api-filter').hidden = false;
    const searchable = [...index].map(([id, item]) => {
      const entry = entries.find(entry => entry.id === id);
      return { item, entry, text: (entry?.textContent ?? item.dataset.apiSearch).toLocaleLowerCase() };
    });
    input.addEventListener('input', () => {
      const query = input.value.trim().toLocaleLowerCase();
      let count = 0;
      for (const { item, entry, text } of searchable) {
        const matches = text.includes(query);
        if (entry) entry.hidden = !matches;
        item.hidden = !matches;
        if (matches) count++;
      }
      root.querySelector('#api-results').textContent = `${count} of ${index.size} exports`;
      const empty = root.querySelector('.api-no-results');
      if (empty) empty.hidden = count !== 0;
    });
  }
  for (const button of root.querySelectorAll('.api-copy, [data-copy-value]')) {
    if (!navigator.clipboard) continue;
    button.hidden = false;
    if (button.matches('[data-copy-value]')) root.querySelector('.api-hash-static').hidden = true;
    const original = button.innerHTML;
    let reset;
    button.addEventListener('click', async () => {
      clearTimeout(reset);
      const status = root.querySelector('.api-copy-status');
      try {
        await navigator.clipboard.writeText(button.dataset.copyValue ?? button.previousElementSibling.textContent);
        button.textContent = 'Copied';
        status.textContent = button.dataset.copyValue ? 'Full source SHA-256 copied.' : 'Signature copied.';
      } catch {
        button.textContent = 'Copy failed';
        status.textContent = 'Copy failed. Select the value to copy it manually.';
      }
      reset = setTimeout(() => { button.innerHTML = original; }, 1800);
    });
  }
  if ('IntersectionObserver' in window) {
    const observer = new IntersectionObserver(items => {
      const visible = items.find(item => item.isIntersecting);
      if (!visible) return;
      for (const [id, item] of index) {
        const link = item.querySelector('a');
        if (id === visible.target.id) link.setAttribute('aria-current', 'location');
        else link.removeAttribute('aria-current');
      }
    }, { rootMargin: '-10% 0px -65% 0px' });
    entries.forEach(entry => observer.observe(entry));
  }
  if (root.hasAttribute('data-source-view')) {
    const updateSourceLocation = () => {
      const selectedLine = Number(location.hash.match(/^#L(\d+)$/)?.[1]);
      let selected;
      for (const item of index.values()) {
        const link = item.querySelector('a');
        link.removeAttribute('aria-current');
        const line = Number(link.hash.match(/^#L(\d+)$/)?.[1]);
        if (line <= selectedLine && (!selected || line > selected.line)) selected = { line, link };
      }
      selected?.link.setAttribute('aria-current', 'location');
    };
    window.addEventListener('hashchange', updateSourceLocation);
    updateSourceLocation();
  }
})();
