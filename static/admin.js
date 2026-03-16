const $=s=>document.querySelector(s);
const tk=()=>sessionStorage.getItem('admin_token')||'';
const hd=()=>({'Authorization':`Bearer ${tk()}`,'Content-Type':'application/json'});

async function auth(){
  const t=$('#admin-token').value.trim();if(!t)return;
  sessionStorage.setItem('admin_token',t);
  const fb=$('#auth-fb');
  try{
    const r=await fetch('/api/admin/promo',{headers:{'Authorization':`Bearer ${t}`}});
    if(r.status===401){fb.className='fb-err';fb.textContent='Invalid token';sessionStorage.removeItem('admin_token');return}
    fb.className='fb-ok';fb.textContent='Connected';
    $('#auth-panel').classList.add('hidden');$('#dash').classList.remove('hidden');loadAll();loadDropPois();
  }catch{fb.className='fb-err';fb.textContent='Connection failed'}
}
if(tk()){$('#admin-token').value=tk();auth()}

async function loadAll(){await Promise.all([loadPromos(),loadSeasons(),loadRegs(),loadHeaderSeason(),loadDrops()])}

async function loadHeaderSeason(){
  try{
    const r=await fetch('/api/season');if(!r.ok)return;
    const s=await r.json();
    const n=String(s.id).padStart(2,'0');
    $('#hdr-sn').textContent='S'+n;
    document.title='Zomboid S'+n+' // Admin';
  }catch{}
}

async function loadPromos(){
  try{
    const r=await fetch('/api/admin/promo',{headers:hd()});if(!r.ok)throw 0;
    const ps=await r.json(),tb=$('#promo-tbody');
    if(!ps.length){tb.innerHTML='<tr><td colspan="5">No promo codes</td></tr>';return}
    tb.innerHTML=ps.map(p=>{
      const u=p.max_uses?`${p.times_used}/${p.max_uses}`:`${p.times_used}`;
      const ac=p.active?'s-ok':'s-no',al=p.active?'Active':'Revoked';
      const b=p.active?`<button class="secondary" data-revoke="${p.code}">Revoke</button>`:'';
      return `<tr><td><code>${p.code}</code></td><td>${p.discount_percent}%</td><td>${u}</td><td class="${ac}">${al}</td><td>${b}</td></tr>`;
    }).join('');
    tb.querySelectorAll('[data-revoke]').forEach(btn=>{
      btn.addEventListener('click',()=>revokePromo(btn.dataset.revoke));
    });
  }catch{$('#promo-tbody').innerHTML='<tr><td colspan="5">Failed to load</td></tr>'}
}

async function createPromo(e){
  e.preventDefault();const fb=$('#promo-fb');fb.textContent='';
  const body={code:$('#p-code').value.trim(),discount_percent:+$('#p-disc').value};
  const mu=$('#p-max').value;if(mu)body.max_uses=+mu;
  try{
    const r=await fetch('/api/admin/promo',{method:'POST',headers:hd(),body:JSON.stringify(body)});
    const d=await r.json();
    if(!r.ok){fb.className='fb-err';fb.textContent=d.error||'Failed';return}
    fb.className='fb-ok';fb.textContent=`Created: ${d.code}`;$('#promo-form').reset();loadPromos();
  }catch{fb.className='fb-err';fb.textContent='Network error'}
}

async function revokePromo(code){
  if(!confirm(`Revoke promo "${code}"?`))return;
  try{const r=await fetch(`/api/admin/promo/${encodeURIComponent(code)}`,{method:'DELETE',headers:hd()});
    if(r.ok||r.status===204)loadPromos()}catch{}
}

async function loadSeasons(){
  try{const r=await fetch('/api/seasons');if(!r.ok)return;const ss=await r.json();
    $('#f-season').innerHTML='<option value="">All</option>'+
      ss.map(s=>`<option value="${s.id}">Season ${s.id} (${s.status})</option>`).join('');
  }catch{}
}

let allRegs=[];

async function loadRegs(){
  try{
    const sid=$('#f-season').value,st=$('#f-status').value;
    let url='/api/admin/registrations';const p=[];
    if(sid)p.push(`season_id=${sid}`);if(st)p.push(`status=${st}`);
    if(p.length)url+='?'+p.join('&');
    const r=await fetch(url,{headers:hd()});
    if(!r.ok){const t=await r.text();throw new Error(`${r.status}: ${t}`)}
    allRegs=await r.json();
    renderRegs();
  }catch(e){$('#reg-tbody').innerHTML=`<tr><td colspan="9">Failed to load: ${e.message||'unknown error'}</td></tr>`}
}

function renderRegs(){
  const q=$('#f-search').value.trim().toLowerCase();
  const filtered=q?allRegs.filter(r=>
    r.zomboid_username.toLowerCase().includes(q)||
    r.id.toLowerCase().includes(q)||
    (r.steam_id&&r.steam_id.toLowerCase().includes(q))||
    (r.promo_code&&r.promo_code.toLowerCase().includes(q))
  ):allRegs;
  const tb=$('#reg-tbody');
  if(!filtered.length){tb.innerHTML='<tr><td colspan="9">No registrations</td></tr>';return}
  tb.innerHTML=filtered.map(r=>{
    const sc=r.status==='confirmed'?'s-ok':r.status==='awaiting_payment'?'s-wait':'s-no';
    const ts=r.confirmed_at||r.created_at;
    const shortId=r.id.substring(0,8);
    const revBtn=r.status!=='expired'?`<button class="secondary danger" data-revoke-reg="${r.id}" data-revoke-name="${r.zomboid_username}">Revoke</button>`:'';
    return `<tr><td><code title="${r.id}">${shortId}</code></td><td>${r.zomboid_username}</td><td>${r.steam_id||'\u2014'}</td><td>${r.season_id}</td><td class="${sc}">${r.status}</td>
      <td><code>${r.amount_wei}</code></td><td>${r.promo_code||'\u2014'}</td>
      <td>${new Date(ts).toLocaleString()}</td><td>${revBtn}</td></tr>`;
  }).join('');
  tb.querySelectorAll('[data-revoke-reg]').forEach(btn=>{
    btn.addEventListener('click',()=>revokeRegistration(btn.dataset.revokeReg,btn.dataset.revokeName));
  });
}

async function revokeRegistration(id,name){
  if(!confirm(`Revoke registration for "${name}"?\n\nThis will expire the registration and remove them from the whitelist.`))return;
  try{
    const r=await fetch(`/api/admin/registrations/${encodeURIComponent(id)}`,{method:'DELETE',headers:hd()});
    if(r.ok||r.status===204)loadRegs();
  }catch{}
}

async function forceRotate(){
  if(!confirm('Force season rotation?\n\n1. Archive current save\n2. Carry forward top killers\n3. Wipe world (PZ generates new)\n4. Start new season\n\nThis cannot be undone.'))return;
  const btn=$('#rotate-btn'),fb=$('#rotate-fb');
  btn.disabled=true;btn.textContent='Rotating...';fb.textContent='';
  try{
    const r=await fetch('/api/admin/rotate',{method:'POST',headers:hd()});const d=await r.json();
    if(!r.ok){fb.className='fb-err';fb.textContent=d.error||'Failed'}
    else{fb.className='fb-ok';fb.textContent='Season rotated!';loadAll()}
  }catch{fb.className='fb-err';fb.textContent='Network error'}
  finally{btn.disabled=false;btn.textContent='Force Rotate Season'}
}

async function loadDrops(){
  try{
    const r=await fetch('/api/admin/supply-drops',{headers:hd()});if(!r.ok)throw 0;
    const drops=await r.json(),tb=$('#drop-tbody');
    if(!drops.length){tb.innerHTML='<tr><td colspan="6">No supply drops</td></tr>';return}
    tb.innerHTML=drops.map(d=>{
      let sc;
      if(d.status==='active')sc='s-ok';
      else if(d.status==='scheduled'||d.status==='announced')sc='s-wait';
      else sc='s-no';
      const shortId=d.id.substring(0,8);
      return `<tr><td><code title="${d.id}">${shortId}</code></td><td>${d.poi_name}</td>
        <td class="${sc}">${d.status}</td><td>${d.loot_tier}</td>
        <td>${new Date(d.scheduled_at).toLocaleString()}</td>
        <td>${d.claimed_by_username||'\u2014'}</td></tr>`;
    }).join('');
  }catch{$('#drop-tbody').innerHTML='<tr><td colspan="6">Failed to load</td></tr>'}
}

async function loadDropPois(){
  try{
    const r=await fetch('/api/supply-drops/pois');if(!r.ok)return;
    const pois=await r.json(),sel=$('#d-poi');
    sel.innerHTML='<option value="">Select POI...</option>';
    pois.forEach(p=>{const o=document.createElement('option');o.value=p;o.textContent=p;sel.appendChild(o)});
  }catch{}
}

async function triggerDrop(e){
  e.preventDefault();const fb=$('#drop-fb');fb.textContent='';
  const poi=$('#d-poi').value;
  if(!poi){fb.className='fb-err';fb.textContent='Select a POI';return}
  const body={poi_name:poi};
  const tier=$('#d-tier').value;if(tier)body.loot_tier=tier;
  try{
    const r=await fetch('/api/admin/supply-drops',{method:'POST',headers:hd(),body:JSON.stringify(body)});
    const d=await r.json();
    if(!r.ok){fb.className='fb-err';fb.textContent=d.error||'Failed';return}
    fb.className='fb-ok';fb.textContent=`Drop scheduled at ${d.poi_name}`;loadDrops();
  }catch{fb.className='fb-err';fb.textContent='Network error'}
}

async function reloadLua(){
  const btn=$('#reload-lua-btn'),fb=$('#reload-lua-fb');
  btn.disabled=true;btn.textContent='Reloading...';fb.textContent='';
  try{
    const r=await fetch('/api/admin/reload-lua',{method:'POST',headers:hd()});const d=await r.json();
    if(!r.ok){fb.className='fb-err';fb.textContent=d.error||'Failed'}
    else{fb.className='fb-ok';fb.textContent='Lua reloaded'}
  }catch{fb.className='fb-err';fb.textContent='Network error'}
  finally{btn.disabled=false;btn.textContent='Reload Lua'}
}

$('#auth-btn').addEventListener('click',auth);
$('#promo-form').addEventListener('submit',createPromo);
$('#drop-form').addEventListener('submit',triggerDrop);
$('#reload-lua-btn').addEventListener('click',reloadLua);
$('#rotate-btn').addEventListener('click',forceRotate);
$('#f-season').addEventListener('change',loadRegs);
$('#f-status').addEventListener('change',loadRegs);
$('#f-search').addEventListener('input',renderRegs);
