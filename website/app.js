const root = document.documentElement;
const saved = localStorage.getItem('theme');
if (saved) root.dataset.theme = saved;
const toggle = document.getElementById('themeToggle');
if (toggle) {
  toggle.addEventListener('click', () => {
    root.dataset.theme = root.dataset.theme === 'dark' ? 'light' : 'dark';
    localStorage.setItem('theme', root.dataset.theme);
  });
}
const terminal = document.querySelector('.terminal code');
if (terminal) {
  const text = terminal.textContent.trim();
  terminal.textContent = '';
  let i = 0;
  const tick = () => {
    terminal.textContent = text.slice(0, i++);
    if (i <= text.length) setTimeout(tick, text[i - 2] === '\n' ? 260 : 18);
  };
  tick();
}
document.querySelectorAll('pre').forEach((pre) => {
  const button = document.createElement('button');
  button.className = 'copy';
  button.textContent = 'copy';
  button.addEventListener('click', async () => {
    await navigator.clipboard.writeText(pre.innerText.replace(/^copy\n/, ''));
    button.textContent = 'copied';
    setTimeout(() => button.textContent = 'copy', 900);
  });
  pre.prepend(button);
});
