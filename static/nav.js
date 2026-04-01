// Shared navigation bar — injected into all pages
(function() {
  const links = [
    ['/', 'home'],
    ['/leaderboard.html', 'leaderboard'],
    ['/account.html', 'account'],
    ['/systems.html', 'systems'],
    ['https://status.princeofcrypto.com', 'status'],
    ['/admin.html', 'admin'],
  ];

  const path = location.pathname;
  const nav = document.querySelector('nav');
  if (!nav) return;

  nav.innerHTML = links.map((item, i) => {
    const [href, label] = item;
    const active = path === href || (href !== '/' && path.endsWith(href));
    const sep = i > 0 ? '<span class="sep">|</span>' : '';
    return sep + '<a href="' + href + '"' + (active ? ' class="active"' : '') + '>' + label + '</a>';
  }).join('');
})();
