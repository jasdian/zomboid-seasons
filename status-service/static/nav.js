// Status service navigation bar
(function() {
  const mainSite = 'https://zomboid.princeofcrypto.com';
  const links = [
    [mainSite + '/', 'home'],
    ['/status.html', 'status'],
    ['/uptime.html', 'uptime'],
    ['/history.html', 'history'],
    ['/admin.html', 'admin'],
    ['/api/status/feed.atom', 'feed'],
  ];

  const path = location.pathname;
  const nav = document.querySelector('nav');
  if (!nav) return;

  nav.innerHTML = links.map((item, i) => {
    const [href, label] = item;
    const active = !href.startsWith('http') && (path === href || path.endsWith(href));
    const sep = i > 0 ? '<span class="sep">|</span>' : '';
    return sep + '<a href="' + href + '"' + (active ? ' class="active"' : '') + '>' + label + '</a>';
  }).join('');
})();
