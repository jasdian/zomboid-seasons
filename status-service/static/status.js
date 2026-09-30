const $=s=>document.querySelector(s);

const STATUS_LABELS={
  operational:'Operational',
  degraded:'Degraded Performance',
  partial_outage:'Partial Outage',
  major_outage:'Major Outage',
  maintenance:'Maintenance'
};

const BANNER_LABELS={
  operational:'All Systems Operational',
  degraded:'Degraded Performance',
  partial_outage:'Partial Outage',
  major_outage:'Major Outage',
  maintenance:'Under Maintenance'
};

function pillHtml(status){
  const label=STATUS_LABELS[status]||status;
  return `<span class="pill pill-${status}">${label}</span>`;
}

function fmtTime(iso){
  const d=new Date(iso);
  const now=new Date();
  const diff=now-d;
  if(diff<60000)return 'just now';
  if(diff<3600000)return Math.floor(diff/60000)+'m ago';
  if(diff<86400000)return Math.floor(diff/3600000)+'h ago';
  return d.toLocaleDateString('en-US',{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit'});
}

function fmtDate(iso){
  return new Date(iso).toLocaleDateString('en-US',{weekday:'long',month:'long',day:'numeric',year:'numeric'});
}

function dateKey(iso){
  return new Date(iso).toISOString().slice(0,10);
}

function esc(s){
  const d=document.createElement('div');d.textContent=s;return d.innerHTML;
}

function uptimeColor(pct){
  if(pct===null||pct===undefined)return '#3a3a3a';
  if(pct>=100)return '#48c78e';
  if(pct>=99)return '#7bb83e';
  if(pct>=95)return '#f0a020';
  return '#e04040';
}

function fmtShortDate(dateStr){
  const d=new Date(dateStr+'T00:00:00Z');
  return d.toLocaleDateString('en-US',{month:'short',day:'numeric'});
}

function renderIncident(inc){
  const comps=inc.component_ids.length?
    `<div class="incident-components">Affecting: ${inc.component_ids.join(', ')}</div>`:'';
  const updates=inc.updates.map(u=>
    `<li class="update-entry">
      <span class="update-status">${u.status}</span>
      <span class="update-msg">${esc(u.message)}</span>
      <span class="update-time">${fmtTime(u.created_at)}</span>
    </li>`
  ).join('');
  return `<div class="incident">
    <div class="incident-header">
      <a href="/incident.html?id=${inc.id}" class="incident-title" style="color:inherit;text-decoration:none">${esc(inc.title)}</a>
      <span class="incident-severity">${inc.severity}</span>
    </div>
    ${comps}
    <ul class="update-timeline">${updates}</ul>
    ${inc.postmortem_body ? `<div style="margin-top:.5rem;padding:.6rem;background:rgba(45,106,79,.08);border-left:2px solid var(--cu);font-size:.75rem">
      <strong style="color:var(--cu);font-size:.65rem;text-transform:uppercase;letter-spacing:.06em">Postmortem</strong><br>
      ${esc(inc.postmortem_body)}
    </div>` : ''}
  </div>`;
}

// ── Uptime Bars ──

function renderUptimeBars(uptimeData){
  const el=$('#uptime');
  if(!uptimeData||!uptimeData.components||!uptimeData.components.length){
    el.innerHTML='<div class="empty-state">No uptime data available</div>';
    return;
  }

  el.innerHTML=uptimeData.components.map(comp=>{
    const pct=comp.uptime_percent_30d;
    // Build date-keyed lookup from API data
    const dayMap={};
    let daysWithData=0;
    (comp.days||[]).forEach(d=>{dayMap[d.date]=d;if(d.total_checks>0)daysWithData++});
    const pctStr=pct!==null&&pct!==undefined?pct.toFixed(2)+'%':'N/A';
    const coverageStr=daysWithData<30?' ('+daysWithData+' day'+(daysWithData!==1?'s':'')+')':'';

    // Generate 30 bars: today-29 ... today
    const today=new Date();
    const barsHtml=Array.from({length:30},(_,i)=>{
      const d=new Date(today);
      d.setDate(d.getDate()-(29-i));
      const dateStr=d.toISOString().slice(0,10);
      const day=dayMap[dateStr];
      if(!day){
        const label=fmtShortDate(dateStr);
        return `<div class="day" style="background:#3a3a3a"><span class="tip">${label} — No data</span></div>`;
      }
      if(day.total_checks===0){
        const label=fmtShortDate(day.date);
        return `<div class="day" style="background:#3a3a3a"><span class="tip">${label} — No data</span></div>`;
      }
      const color=uptimeColor(day.uptime_percent);
      const label=fmtShortDate(day.date);
      const incText=day.incident_count===1?'1 incident':(day.incident_count||0)+' incidents';
      const pctLabel=day.uptime_percent!==null?day.uptime_percent.toFixed(2)+'%':'N/A';
      return `<div class="day" style="background:${color}"><span class="tip">${label} — ${pctLabel} — ${incText}</span></div>`;
    }).join('');

    return `<div class="uptime-component">
      <div class="uptime-header">
        <span class="name">${esc(comp.name)}</span>
        <span class="pct">${pctStr} uptime${coverageStr}</span>
      </div>
      <div class="uptime-bar">${barsHtml}</div>
      <div class="uptime-footer">
        <span>30 days ago</span>
        <span>Today</span>
      </div>
    </div>`;
  }).join('');
}

// ── Main Load ──

async function loadStatus(){
  try{
    const [statusRes, uptimeRes, maintRes]=await Promise.all([
      fetch('/api/status'),
      fetch('/api/status/uptime'),
      fetch('/api/status/maintenance')
    ]);
    if(!statusRes.ok)throw 0;
    const data=await statusRes.json();
    const uptimeData=uptimeRes.ok?await uptimeRes.json():null;
    const maintRaw=maintRes.ok?await maintRes.json():null;
    // Support both old (array) and new ({active,completed}) response shapes
    const activeMaint=Array.isArray(maintRaw)?maintRaw:(maintRaw?.active||[]);
    const completedMaint=Array.isArray(maintRaw)?[]:(maintRaw?.completed||[]);

    // Banner
    const banner=$('#status-banner');
    banner.className='status-banner st-'+data.overall_status;
    banner.textContent=BANNER_LABELS[data.overall_status]||data.overall_status;

    // Maintenance banners
    const mb=$('#maintenance-banners');
    if(activeMaint.length){
      mb.innerHTML=activeMaint.map(m=>{
        const label=m.status==='in_progress'?'In Progress':'Scheduled';
        const start=new Date(m.scheduled_start).toLocaleString('en-US',{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit'});
        const end=new Date(m.scheduled_end).toLocaleString('en-US',{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit'});
        return `<div class="maintenance-banner">
          <strong>${label}:</strong> ${esc(m.title)} — ${start} to ${end}
        </div>`;
      }).join('');
    }else{
      mb.innerHTML='';
    }

    // Components (grouped)
    const cl=$('#comp-list');
    if(data.components.length){
      const groupMap={};
      (data.component_groups||[]).forEach(g=>{groupMap[g.id]=g.name});

      // Group components
      const grouped={};
      const ungrouped=[];
      data.components.forEach(c=>{
        if(c.group_id && groupMap[c.group_id]){
          if(!grouped[c.group_id])grouped[c.group_id]=[];
          grouped[c.group_id].push(c);
        } else {
          ungrouped.push(c);
        }
      });

      let html='';
      // Render grouped components
      for(const [gid, comps] of Object.entries(grouped)){
        html+=`<li class="comp-group-header" style="font-size:.7rem;color:var(--dim);text-transform:uppercase;letter-spacing:.1em;padding:.8rem 0 .3rem;border-bottom:1px solid var(--brd)">${esc(groupMap[gid])}</li>`;
        html+=comps.map(c=>
          `<li class="comp-item">
            <div><div class="comp-name">${esc(c.name)}</div><div class="comp-desc">${esc(c.description)}</div></div>
            ${pillHtml(c.status)}
          </li>`
        ).join('');
      }
      // Render ungrouped
      html+=ungrouped.map(c=>
        `<li class="comp-item">
          <div><div class="comp-name">${esc(c.name)}</div><div class="comp-desc">${esc(c.description)}</div></div>
          ${pillHtml(c.status)}
        </li>`
      ).join('');
      cl.innerHTML=html;
    }else{
      cl.innerHTML='<li class="empty-state">No components configured</li>';
    }

    // Uptime bars
    renderUptimeBars(uptimeData);

    // Active incidents
    const ap=$('#active-panel');
    const ai=$('#active-incidents');
    if(data.active_incidents.length){
      ap.style.display='';
      ai.innerHTML=data.active_incidents.map(renderIncident).join('');
    }else{
      ap.style.display='none';
    }

    // Past incidents
    const pi=$('#past-incidents');
    if(data.past_incidents.length){
      const groups={};
      data.past_incidents.forEach(inc=>{
        const dk=dateKey(inc.created_at);
        if(!groups[dk])groups[dk]=[];
        groups[dk].push(inc);
      });
      const sortedKeys=Object.keys(groups).sort().reverse();
      pi.innerHTML=sortedKeys.map(dk=>
        `<div class="date-group">
          <div class="date-label">${fmtDate(dk+'T00:00:00Z')}</div>
          ${groups[dk].map(renderIncident).join('')}
        </div>`
      ).join('');
    }else{
      pi.innerHTML='<div class="empty-state">No incidents in the past 15 days</div>';
    }

    // Past maintenance
    const pm=$('#past-maintenance');
    if(pm){
      if(completedMaint.length){
        pm.innerHTML=completedMaint.map(m=>{
          const start=new Date(m.scheduled_start);
          const end=new Date(m.actual_end||m.scheduled_end);
          const durMs=end-start;
          const durMin=Math.round(durMs/60000);
          const durStr=durMin>=60?Math.floor(durMin/60)+'h '+durMin%60+'m':durMin+'m';
          const dateStr=start.toLocaleDateString('en-US',{month:'short',day:'numeric'});
          const startTime=start.toLocaleTimeString('en-US',{hour:'2-digit',minute:'2-digit'});
          const endTime=end.toLocaleTimeString('en-US',{hour:'2-digit',minute:'2-digit'});
          const comps=(m.component_ids||[]).join(', ');
          return `<div class="maint-history-item">
            <div class="maint-history-header">
              <span class="maint-history-title">${esc(m.title)}</span>
              <span class="maint-history-dur">${durStr}</span>
            </div>
            <div class="maint-history-meta">${dateStr}, ${startTime} — ${endTime}${comps?' · '+comps:''}</div>
          </div>`;
        }).join('');
      }else{
        pm.innerHTML='<div class="empty-state">No maintenance in the past 15 days</div>';
      }
    }
  }catch{
    $('#comp-list').innerHTML='<li class="empty-state">Failed to load status</li>';
  }
}

loadStatus();
setInterval(loadStatus,60000);
