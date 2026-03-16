const $ = s => document.querySelector(s);
const REFRESH_MS = 60 * 1000;

async function loadSeasons() {
    const r = await fetch('/api/seasons');
    if (!r.ok) return [];
    return r.json();
}

async function loadLeaderboard(seasonId) {
    const params = new URLSearchParams();
    if (seasonId) params.set('season_id', seasonId);
    const url = '/api/leaderboard' + (params.toString() ? '?' + params : '');
    const r = await fetch(url);
    if (!r.ok) throw new Error('HTTP ' + r.status);
    return r.json();
}

function rankClass(i) {
    if (i === 0) return 'rank-gold';
    if (i === 1) return 'rank-silver';
    if (i === 2) return 'rank-bronze';
    return '';
}

function fmtNum(n) { return Number(n).toLocaleString() }
function fmtDays(d) { return d != null ? d.toFixed(1) : '0' }

function renderLeaderboard(data) {
    $('#lb-title').textContent = 'Season ' + data.season_id + ' Leaderboard';

    const tbody = $('#lb-tbody');
    const empty = $('#lb-empty');

    if (!data.entries.length) {
        tbody.innerHTML = '';
        empty.classList.remove('hidden');
        $('#lb-meta').textContent = '';
        return;
    }
    empty.classList.add('hidden');

    tbody.innerHTML = data.entries.map((e, i) => {
        const title = 'Kill pts: ' + (e.p_kills||0).toFixed(0) +
            ' | Survival pts: ' + (e.p_survival||0).toFixed(0) +
            ' | Online pts: ' + (e.p_online||0).toFixed(0);
        return '<tr class="' + rankClass(i) + '" title="' + title + '">' +
            '<td class="rank-col">' + e.rank + '</td>' +
            '<td>' + e.zomboid_username + '</td>' +
            '<td class="num-col">' + fmtNum(e.season_score || e.zombie_kills) + '</td>' +
            '<td class="num-col">' + fmtNum(e.best_kills || e.zombie_kills) + '</td>' +
            '<td class="num-col">' + fmtDays(e.best_survival_days) + 'd</td>' +
            '<td class="num-col">' + (e.days_logged || 0) + 'd</td>' +
            '</tr>';
    }).join('');

    const t = new Date(data.fetched_at);
    $('#lb-meta').textContent = 'Last updated: ' + t.toLocaleTimeString();
}

async function refresh() {
    const sel = $('#season-sel');
    try {
        const data = await loadLeaderboard(sel.value || null);
        renderLeaderboard(data);
    } catch {
        $('#lb-tbody').innerHTML = '<tr><td colspan="6" style="color:var(--ered)">Failed to load</td></tr>';
    }
}

async function init() {
    try {
        const seasons = await loadSeasons();
        const sel = $('#season-sel');
        sel.innerHTML = '<option value="">Current Season</option>' +
            seasons.map(s =>
                '<option value="' + s.id + '">Season ' + s.id + ' (' + s.status + ')</option>'
            ).join('');

        sel.addEventListener('change', refresh);

        await refresh();
        setInterval(refresh, REFRESH_MS);
    } catch {
        $('#lb-title').textContent = 'Leaderboard Unavailable';
    }
}

init();
