const $ = s => document.querySelector(s);

async function fetchRetry(url,opts={},retries=3){
  for(let i=0;i<retries;i++){
    try{
      const r=await fetch(url,opts);
      if(r.status===429){await new Promise(res=>setTimeout(res,Math.min(1000*Math.pow(2,i),8000)));continue}
      return r;
    }catch(e){if(i===retries-1)throw e;await new Promise(res=>setTimeout(res,1000))}
  }
  return fetch(url,opts);
}

async function lookup() {
    const steamId = $('#steam-input').value.trim();
    const errEl = $('#error');
    const panel = $('#acct-panel');

    errEl.classList.add('hidden');
    panel.classList.add('hidden');

    if (!steamId || !/^\d{17}$/.test(steamId)) {
        errEl.textContent = 'Enter a valid 17-digit Steam ID';
        errEl.classList.remove('hidden');
        return;
    }

    try {
        const r = await fetchRetry('/api/account/' + steamId);
        if (r.status === 404) {
            errEl.textContent = 'Account not found. Play on the server to create one.';
            errEl.classList.remove('hidden');
            return;
        }
        if (!r.ok) throw new Error('HTTP ' + r.status);
        const data = await r.json();
        renderAccount(data);
    } catch (e) {
        errEl.textContent = 'Failed to load account: ' + e.message;
        errEl.classList.remove('hidden');
    }
}

function renderAccount(data) {
    const panel = $('#acct-panel');
    panel.classList.remove('hidden');

    $('#acct-title').textContent = 'Account';
    $('#balance').textContent = Number(data.balance).toLocaleString();

    const bonusEl = $('#daily-bonus');
    if (data.daily_online_bonus) {
        bonusEl.className = 'daily-bonus earned';
        bonusEl.textContent = 'Online Bonus: Earned';
    } else {
        bonusEl.className = 'daily-bonus pending';
        bonusEl.textContent = 'Online Bonus: Pending';
    }
    bonusEl.classList.remove('hidden');

    const tbody = $('#history-tbody');
    const noHistory = $('#no-history');

    if (!data.seasons || !data.seasons.length) {
        tbody.innerHTML = '';
        noHistory.classList.remove('hidden');
        return;
    }
    noHistory.classList.add('hidden');

    tbody.innerHTML = data.seasons.map(s =>
        '<tr>' +
        '<td>Season ' + s.season_id + '</td>' +
        '<td class="num-col">' + Number(s.score).toLocaleString() + '</td>' +
        '<td class="num-col">' + s.characters + '</td>' +
        '</tr>'
    ).join('');
}

$('#lookup-btn').addEventListener('click', lookup);
$('#steam-input').addEventListener('keydown', e => { if (e.key === 'Enter') lookup() });
