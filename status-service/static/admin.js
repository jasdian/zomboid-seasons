const $=s=>document.querySelector(s);
const tk=()=>sessionStorage.getItem('status_admin_token')||'';
const hd=()=>({'Authorization':`Bearer ${tk()}`,'Content-Type':'application/json'});

function esc(s){const d=document.createElement('div');d.textContent=s;return d.innerHTML}

// ── Auth ──

async function auth(){
  const t=$('#admin-token').value.trim();if(!t)return;
  sessionStorage.setItem('status_admin_token',t);
  const fb=$('#auth-fb');
  try{
    const r=await fetch('/api/admin/incidents',{headers:{'Authorization':`Bearer ${t}`}});
    if(r.status===401){fb.className='fb-err';fb.textContent='Invalid token';sessionStorage.removeItem('status_admin_token');return}
    fb.className='fb-ok';fb.textContent='Connected';
    $('#auth-panel').classList.add('hidden');$('#dash').classList.remove('hidden');loadAll();
  }catch{fb.className='fb-err';fb.textContent='Connection failed'}
}
if(tk()){$('#admin-token').value=tk();auth()}

async function loadAll(){await Promise.all([loadIncidents(),loadCompOverrides(),loadMaintenance()])}

// ── Incidents ──

async function loadIncidentComponents(){
  try{
    const r=await fetch('/api/status');if(!r.ok)return;
    const data=await r.json();
    for(const el of [document.querySelector('#inc-components'),document.querySelector('#maint-components')]){
      if(!el)continue;
      el.innerHTML=data.components.map(c=>
        `<label><input type="checkbox" value="${c.id}"> ${c.name}</label>`
      ).join('');
    }
  }catch{}
}

async function loadIncidents(){
  loadIncidentComponents();
  try{
    const r=await fetch('/api/admin/incidents',{headers:hd()});if(!r.ok)throw 0;
    const incs=await r.json();
    const el=$('#active-incidents-admin');
    const active=incs.filter(i=>i.status!=='resolved');
    if(!active.length){el.innerHTML='<div class="empty-state">No active incidents</div>';}
    else { el.innerHTML=active.map(inc=>{
      const updates=inc.updates.map(u=>
        `<div><strong>${u.status}</strong> — ${esc(u.message)} <span class="ts">${new Date(u.created_at).toLocaleString()}</span></div>`
      ).join('');
      return `<div class="inc-card">
        <div class="inc-card-header">
          <span class="inc-card-title">${esc(inc.title)}</span>
          <span class="inc-card-meta">${inc.severity} · ${inc.status}${inc.auto_detected?' · auto':''}
            <span class="ts">${new Date(inc.created_at).toLocaleString()}</span></span>
        </div>
        ${updates?`<div class="inc-updates">${updates}</div>`:''}
        <div class="inc-add-update">
          <select data-inc-status="${inc.id}">
            <option value="investigating">Investigating</option>
            <option value="identified">Identified</option>
            <option value="monitoring">Monitoring</option>
            <option value="update">Update</option>
            <option value="resolved">Resolved</option>
          </select>
          <input type="text" data-inc-msg="${inc.id}" placeholder="Update message...">
          <button class="secondary" data-add-update="${inc.id}">Post</button>
        </div>
        <div class="inc-actions">
          <button class="secondary danger" data-resolve-inc="${inc.id}">Resolve</button>
          <button class="secondary danger" data-delete-inc="${inc.id}">Delete</button>
        </div>
      </div>`;
    }).join('');
    el.querySelectorAll('[data-add-update]').forEach(btn=>{
      btn.addEventListener('click',()=>addIncidentUpdate(btn.dataset.addUpdate));
    });
    el.querySelectorAll('[data-resolve-inc]').forEach(btn=>{
      btn.addEventListener('click',()=>resolveIncident(btn.dataset.resolveInc));
    });
    el.querySelectorAll('[data-delete-inc]').forEach(btn=>{
      btn.addEventListener('click',()=>deleteIncident(btn.dataset.deleteInc));
    });
    } // end active else
    // Resolved incidents
    const fifteenDaysAgo = new Date(Date.now() - 15 * 24 * 3600000);
    const resolved = incs.filter(i => i.status === 'resolved' && new Date(i.resolved_at || i.created_at) >= fifteenDaysAgo).slice(0, 10);
    const rel = $('#resolved-incidents-admin');
    if (!resolved.length) { rel.innerHTML = '<div class="empty-state">No resolved incidents</div>'; }
    else {
      rel.innerHTML = resolved.map(inc => {
        const hasPostmortem = !!inc.postmortem_body;
        return `<div class="inc-card" style="opacity:.7">
          <div class="inc-card-header">
            <span class="inc-card-title">${esc(inc.title)}</span>
            <span class="inc-card-meta">${inc.severity} · resolved
              <span class="ts">${new Date(inc.created_at).toLocaleString()}</span></span>
          </div>
          ${hasPostmortem
            ? `<div class="inc-updates" style="font-size:.75rem;color:var(--dim)">Postmortem published ${new Date(inc.postmortem_published_at).toLocaleString()}</div>`
            : `<div class="inc-add-update">
                <textarea data-pm-body="${inc.id}" rows="3" placeholder="Write postmortem..." style="width:100%;margin-bottom:.3rem"></textarea>
                <button class="secondary" data-publish-pm="${inc.id}">Publish Postmortem</button>
              </div>`}
        </div>`;
      }).join('');
      rel.querySelectorAll('[data-publish-pm]').forEach(btn => {
        btn.addEventListener('click', () => publishPostmortem(btn.dataset.publishPm));
      });
    }
  }catch{$('#active-incidents-admin').innerHTML='<div class="fb-err">Failed to load</div>'}
}

async function createIncident(e){
  e.preventDefault();const fb=$('#incident-fb');fb.textContent='';
  const title=$('#inc-title').value.trim();
  const severity=$('#inc-severity').value;
  const message=$('#inc-message').value.trim();
  const checks=document.querySelectorAll('#inc-components input:checked');
  const component_ids=[...checks].map(c=>c.value);
  if(!title||!message){fb.className='fb-err';fb.textContent='Title and message required';return}
  try{
    const r=await fetch('/api/admin/incidents',{method:'POST',headers:hd(),
      body:JSON.stringify({title,severity,component_ids,message})});
    const d=await r.json();
    if(!r.ok){fb.className='fb-err';fb.textContent=d.error||'Failed';return}
    fb.className='fb-ok';fb.textContent='Incident created';
    $('#incident-form').reset();loadIncidents();
  }catch{fb.className='fb-err';fb.textContent='Network error'}
}

async function addIncidentUpdate(incId){
  const sel=document.querySelector(`[data-inc-status="${incId}"]`);
  const inp=document.querySelector(`[data-inc-msg="${incId}"]`);
  const status=sel.value,message=inp.value.trim();
  if(!message)return;
  try{
    const r=await fetch(`/api/admin/incidents/${incId}/updates`,{method:'POST',headers:hd(),
      body:JSON.stringify({status,message})});
    if(r.ok){inp.value='';loadIncidents()}
  }catch{}
}

async function resolveIncident(incId){
  if(!confirm('Resolve this incident?'))return;
  try{
    await fetch(`/api/admin/incidents/${incId}/updates`,{method:'POST',headers:hd(),
      body:JSON.stringify({status:'resolved',message:'This incident has been resolved.'})});
    loadIncidents();
  }catch{}
}

async function deleteIncident(incId){
  if(!confirm('Delete this incident? This cannot be undone.'))return;
  try{
    const r=await fetch(`/api/admin/incidents/${incId}`,{method:'DELETE',headers:hd()});
    if(r.ok||r.status===204)loadIncidents();
  }catch{}
}

async function publishPostmortem(incId){
  const textarea=document.querySelector(`[data-pm-body="${incId}"]`);
  const body=textarea.value.trim();
  if(!body)return;
  try{
    const r=await fetch(`/api/admin/incidents/${incId}/postmortem`,{
      method:'POST',headers:hd(),body:JSON.stringify({body})
    });
    if(r.ok||r.status===204)loadIncidents();
  }catch{}
}

// ── Component Overrides ──

async function loadCompOverrides(){
  try{
    const r=await fetch('/api/status');if(!r.ok)return;
    const data=await r.json();
    const el=$('#comp-overrides');
    el.innerHTML=data.components.map(c=>
      `<div class="override-row">
        <span class="override-name">${esc(c.name)}</span>
        <select data-override-comp="${c.id}">
          <option value="">Auto (from incidents)</option>
          <option value="operational"${c.status==='operational'?' selected':''}>Operational</option>
          <option value="degraded"${c.status==='degraded'?' selected':''}>Degraded</option>
          <option value="partial_outage"${c.status==='partial_outage'?' selected':''}>Partial Outage</option>
          <option value="major_outage"${c.status==='major_outage'?' selected':''}>Major Outage</option>
          <option value="maintenance"${c.status==='maintenance'?' selected':''}>Maintenance</option>
        </select>
        <button class="secondary" data-set-override="${c.id}">Set</button>
      </div>`
    ).join('');
    el.querySelectorAll('[data-set-override]').forEach(btn=>{
      btn.addEventListener('click',()=>setOverride(btn.dataset.setOverride));
    });
  }catch{}
}

async function setOverride(compId){
  const sel=document.querySelector(`[data-override-comp="${compId}"]`);
  const val=sel.value||null;
  try{
    await fetch(`/api/admin/components/${compId}/override`,{method:'POST',headers:hd(),
      body:JSON.stringify({status:val})});
    loadCompOverrides();
  }catch{}
}

// ── Maintenance ──

function renderMaintCard(m){
  const sc=m.status==='in_progress'?'s-ok':m.status==='scheduled'?'s-wait':'s-no';
  return `<div class="inc-card">
    <div class="inc-card-header">
      <span class="inc-card-title">${esc(m.title)}</span>
      <span class="inc-card-meta"><span class="${sc}">${m.status}</span></span>
    </div>
    <div class="maint-details">
      ${m.description?`<div>${esc(m.description)}</div>`:''}
      <div class="ts">Start: ${new Date(m.scheduled_start).toLocaleString()} — End: ${new Date(m.scheduled_end).toLocaleString()}</div>
      <div class="ts">Components: ${(m.component_ids||[]).join(', ')||'none'}</div>
    </div>
    ${m.status!=='completed'?`<div class="inc-actions">
      ${m.status==='scheduled'?`<button class="secondary" data-maint-start="${m.id}">Start Now</button>`:''}
      <button class="secondary" data-maint-complete="${m.id}">Complete</button>
    </div>`:''}
  </div>`;
}

async function loadMaintenance(){
  try{
    const r=await fetch('/api/admin/maintenance',{headers:hd()});if(!r.ok)throw 0;
    const items=await r.json();
    const el=$('#maint-list');
    if(!items.length){el.innerHTML='<div class="empty-state">No maintenance windows</div>';return}
    const active=items.filter(m=>m.status!=='completed');
    const fifteenDaysAgo=new Date(Date.now()-15*24*3600000);
    const completed=items.filter(m=>m.status==='completed'&&new Date(m.actual_end||m.scheduled_end)>=fifteenDaysAgo);
    let html='';
    if(active.length){
      html+=active.map(renderMaintCard).join('');
    }else{
      html+='<div class="empty-state">No active or scheduled maintenance</div>';
    }
    if(completed.length){
      html+=`<details style="margin-top:.8rem"><summary style="cursor:pointer;color:var(--dim);font-size:.72rem;letter-spacing:.08em;text-transform:uppercase;list-style:none">Completed — past 15 days (${completed.length})</summary>`;
      html+=completed.map(renderMaintCard).join('');
      html+='</details>';
    }
    el.innerHTML=html;
    el.querySelectorAll('[data-maint-start]').forEach(btn=>{
      btn.addEventListener('click',()=>patchMaintenance(btn.dataset.maintStart,'in_progress'));
    });
    el.querySelectorAll('[data-maint-complete]').forEach(btn=>{
      btn.addEventListener('click',()=>patchMaintenance(btn.dataset.maintComplete,'completed'));
    });
  }catch{$('#maint-list').innerHTML='<div class="fb-err">Failed to load</div>'}
}

async function patchMaintenance(id,status){
  try{
    await fetch(`/api/admin/maintenance/${id}`,{method:'PATCH',headers:hd(),
      body:JSON.stringify({status})});
    loadMaintenance();loadCompOverrides();
  }catch{}
}

function getScheduledStart(){
  const preset=$('#m-start-preset').value;
  if(preset==='now')return new Date();
  if(preset==='15m')return new Date(Date.now()+15*60000);
  if(preset==='1h')return new Date(Date.now()+3600000);
  const v=$('#m-start').value;
  return v?new Date(v):null;
}

function getScheduledEnd(start){
  const dur=$('#m-duration').value;
  if(dur==='custom'){
    const v=$('#m-end').value;
    return v?new Date(v):null;
  }
  return new Date(start.getTime()+parseInt(dur)*60000);
}

$('#m-start-preset').addEventListener('change',function(){
  const wrap=$('#m-start-custom-wrap');
  wrap.classList.toggle('hidden',this.value!=='custom');
});

$('#m-duration').addEventListener('change',function(){
  const wrap=$('#m-end-custom-wrap');
  wrap.classList.toggle('hidden',this.value!=='custom');
});

async function createMaintenance(e){
  e.preventDefault();const fb=$('#maint-fb');fb.textContent='';
  const title=$('#m-title').value.trim();
  const description=$('#m-desc').value.trim();
  const checks=document.querySelectorAll('#maint-components input:checked');
  const component_ids=[...checks].map(c=>c.value);
  const start=getScheduledStart();
  if(!title||!start){fb.className='fb-err';fb.textContent='Title and start required';return}
  const end=getScheduledEnd(start);
  if(!end){fb.className='fb-err';fb.textContent='End time required';return}
  if(end<=start){fb.className='fb-err';fb.textContent='End must be after start';return}
  try{
    const r=await fetch('/api/admin/maintenance',{method:'POST',headers:hd(),
      body:JSON.stringify({title,description,component_ids,
        scheduled_start:start.toISOString(),
        scheduled_end:end.toISOString()})});
    const d=await r.json();
    if(!r.ok){fb.className='fb-err';fb.textContent=d.error||'Failed';return}
    fb.className='fb-ok';fb.textContent='Maintenance scheduled';
    $('#maint-form').reset();
    $('#m-start-custom-wrap').classList.add('hidden');
    $('#m-end-custom-wrap').classList.add('hidden');
    loadMaintenance();
  }catch{fb.className='fb-err';fb.textContent='Network error'}
}

// ── Event Listeners ──

$('#auth-btn').addEventListener('click',auth);
$('#incident-form').addEventListener('submit',createIncident);
$('#maint-form').addEventListener('submit',createMaintenance);
