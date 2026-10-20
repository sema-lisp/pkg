(() => {
  const root = document.getElementById('api-reference');
  if (!root) return;
  const input = root.querySelector('#api-search');
  const entries = [...root.querySelectorAll('[data-api-entry]')];
  const index = new Map([...root.querySelectorAll('[data-api-index]')].map(item => [item.dataset.apiIndex, item]));
  if (input) {
    root.querySelector('.api-filter').hidden = false;
    const searchable = entries.map(entry => ({ entry, text: entry.textContent.toLocaleLowerCase() }));
    input.addEventListener('input', () => {
      const query = input.value.trim().toLocaleLowerCase();
      let count = 0;
      for (const { entry, text } of searchable) {
        const matches = text.includes(query);
        entry.hidden = !matches;
        index.get(entry.id).hidden = !matches;
        if (matches) count++;
      }
      root.querySelector('#api-results').textContent = `${count} of ${entries.length} exports`;
      root.querySelector('.api-no-results').hidden = count !== 0;
    });
  }
  for (const button of root.querySelectorAll('.api-copy')) {
    if (!navigator.clipboard) continue;
    button.hidden = false;
    button.addEventListener('click', async () => {
      try {
        await navigator.clipboard.writeText(button.previousElementSibling.textContent);
        button.textContent = 'Copied';
      } catch {
        button.textContent = 'Copy failed';
      }
      setTimeout(() => { button.textContent = 'Copy'; }, 1800);
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
})();
