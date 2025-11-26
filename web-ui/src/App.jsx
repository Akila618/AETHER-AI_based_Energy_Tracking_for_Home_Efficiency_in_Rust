import './App.css'
import aether_logo from './assets/aether_dark.png'
import { useState, useEffect, useRef } from "react";

function App() {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [loggedIn, setLoggedIn] = useState(false)
  const [error, setError] = useState('')
  const [messages, setMessages] = useState([])
  const [chatInput, setChatInput] = useState('')
  const [prediction, setPrediction] = useState(null)
  const [liveUpdates, setLiveUpdates] = useState([])
  const [alerts, setAlerts] = useState([])
  const [recs, setRecs] = useState([])
  const [liveTotal, setLiveTotal] = useState(null)
  const [dailyKwh, setDailyKwh] = useState(null)
  const [dailyDelta, setDailyDelta] = useState(null)
  const chatMessagesRef = useRef(null)

  function handleSubmit(e) {
    e.preventDefault()
    setError('')
    if (username.trim() === '' || password.trim() === '') {
      setError('Please enter username and password')
      return
    }
    setLoggedIn(true)
  }

  function handleLogout() {
    setLoggedIn(false)
    setUsername('')
    setPassword('')
    setError('')
  }

  useEffect(() => {
    if (!loggedIn) return;

    const url = `${location.protocol === 'https:' ? 'wss' : 'ws'}://127.0.0.1:3000/ws`;
    const ws = new WebSocket(url);

    ws.onopen = () => console.log('ws open');
    ws.onmessage = (evt) => {
      try {
        const obj = JSON.parse(evt.data);
        const t = obj.type || 'state';
        if (t === 'state') {
          setLiveUpdates(prev => [obj, ...prev].slice(0, 50));
        } else if (t === 'alert') {
          
          setAlerts(prev => [obj, ...prev].slice(0, 30));
        } else if (t === 'recommendation') {
          setRecs(prev => [obj, ...prev].slice(0, 30));
          if (obj.daily_kwh !== undefined) setDailyKwh(typeof obj.daily_kwh === 'number' ? obj.daily_kwh : Number(obj.daily_kwh || 0));
          if (obj.delta_pct !== undefined) setDailyDelta(typeof obj.delta_pct === 'number' ? obj.delta_pct : Number(obj.delta_pct || 0));
        } else if (t === 'info') {
         
          setRecs(prev => [obj, ...prev].slice(0, 30));
          if (obj.daily_kwh !== undefined) setDailyKwh(typeof obj.daily_kwh === 'number' ? obj.daily_kwh : Number(obj.daily_kwh || 0));
          if (obj.delta_pct !== undefined) setDailyDelta(typeof obj.delta_pct === 'number' ? obj.delta_pct : Number(obj.delta_pct || 0));
        } else if (t === 'live_total') {
          
          const total = typeof obj.total_watts === 'number' ? obj.total_watts : Number(obj.total_watts || 0);
          setLiveTotal(total);
        } else {
          
          setMessages(prev => [{from:'System', text: JSON.stringify(obj)} , ...prev].slice(0,20));
        }
      } catch(e) { console.error('ws message parse', e); }
    };
    ws.onclose = () => console.log('ws closed');
    ws.onerror = (err) => console.error('WebSocket error:', err);

    return () => ws.close();
  }, [loggedIn]);

  
  useEffect(() => {
    if (chatMessagesRef.current) {
      chatMessagesRef.current.scrollTop = chatMessagesRef.current.scrollHeight;
    }
  }, [messages]);
 
  if (!loggedIn) {
    return (
      <div className="login-container">
        <div className="login-card">
          <img src={aether_logo} alt="AETHER Logo" className="logo" />
          <h2>Welcome to AETHER</h2>
          <h2>Agentic Energy Tracking System for Home Efficiency in Rust and React</h2>
          <div className="login-form-container">
            <div className="login-form">
              <label>
                Username
                <input
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  type="text"
                  name="username"
                  autoComplete="username"
                />
              </label>

              <label>
                Password
                <input
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  type="password"
                  name="password"
                  autoComplete="current-password"
                  onKeyDown={(e) => e.key === 'Enter' && handleSubmit(e)}
                />
              </label>

              {error && <div className="error">{error}</div>}

              <button onClick={handleSubmit} className="btn primary btn-full">Login</button>
            </div>
          </div>
        </div>
      </div>
    )
  }

  function sendMsg() {
    if (!chatInput.trim()) return
    const newMsg = { from: 'You', text: chatInput }
    setMessages(prev => [newMsg, ...prev].slice(0, 50))
    const q = chatInput
    
    try {
      if (typeof setChatInput === 'function') {
        setChatInput('')
      } else {
        console.warn('[CHAT] setChatInput is not a function:', typeof setChatInput)
        
        const el = document.querySelector('.chat-input-container input') || document.querySelector('input[placeholder="Type your message..."]')
        if (el) el.value = ''
      }
    } catch (e) {
      console.warn('[CHAT] Error clearing chat input', e)
    }
    // send to backend chat API
    (async () => {
      try {
        const res = await fetch('http://127.0.0.1:3000/api/chat', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ query: q })
        })
        console.log('Chat fetch status', res.status, res.statusText)
        const raw = await res.text();
        let data = {};
        try { data = JSON.parse(raw); } catch(e) { data = { reply: raw }; }
        const replyText = data.reply || data.message || (typeof data === 'string' ? data : JSON.stringify(data));
        if (res.ok) {
          setMessages(prev => [{ from: 'Agent', text: replyText }, ...prev].slice(0, 50))
        } else {
          setMessages(prev => [{ from: 'Agent', text: replyText || (`Chat error: ${res.status}`) }, ...prev].slice(0, 50))
        }
      } catch (e) {
        setMessages(prev => [{ from: 'Agent', text: `Chat request failed: ${e.message}` }, ...prev].slice(0, 50))
      }
    })()
  }

  function handleKey(e) {
    if (e.key === 'Enter') {
      e.preventDefault();
      sendMsg();
    }
  }

  async function handlePrediction() {
    const y = document.getElementById('pred-year').value
    const m = document.getElementById('pred-month').value
    setPrediction(null)
    try {
      const res = await fetch(`http://127.0.0.1:3000/api/predictions?year=${y}&month=${m}`)
      if (!res.ok) throw new Error(await res.text())
      const data = await res.json()
      setPrediction({ 
        year: data.year, 
        month: data.month, 
        monthly_kwh: Number(data.monthly_kwh), 
        monthly_cost: Number(data.monthly_cost), 
        daily: data.daily_watts 
      })
      setMessages(prev => [{
        from:'System', 
        text: `Estimated ${y}-${m}: ${Number(data.monthly_kwh).toFixed(2)} kWh, Cost LKR ${Number(data.monthly_cost).toFixed(2)}`
      }, ...prev])
    } catch (e) {
      setMessages(prev => [{from:'System', text:`Prediction failed: ${e.message}`}, ...prev])
    }
  }

  return (
    <div className="app-root">
      {/* Header */}
      <header className="app-header">
        <div>
          <div className="brand">
            <div className="logo small"><img src={aether_logo} alt="AETHER Logo" className="logo_dashboard" /></div>
            <div>
              <h1>AETHER Dashboard</h1>
              <h2>AI-Powered Energy Management System</h2>
            </div>
          </div>
          <button onClick={handleLogout} className="btn secondary">Logout</button>
        </div>
      </header>

      {/* Main Dashboard */}
      <main className="dashboard">
        {/* Top Stats Bar */}
        <div className="stats-bar">
          {/* Live Wattage Monitor */}
          <div className="stat-card gradient">
            <div className="stat-header">
              <h3>Current Power Usage</h3>
              <div className="pulse-indicator"></div>
            </div>
            <div className="stat-value">
              {liveTotal !== null ? liveTotal.toFixed(0) : '—'}
              <span className="unit">W</span>
            </div>
            <div className="stat-label">Real-time monitoring</div>
          </div>

          {/* Daily Average */}
          <div className="stat-card">
            <div className="stat-header">
              <h3>Daily Average</h3>
            </div>
            <div className="stat-value">
              {dailyKwh !== null ? dailyKwh.toFixed(2) : '—'}
              <span className="unit">kWh</span>
            </div>
            <div className="stat-change">
              {dailyDelta !== null ? (
                (dailyDelta < 0 ? `↓ ${Math.abs(dailyDelta).toFixed(0)}% from yesterday` : `↑ ${dailyDelta.toFixed(0)}% from yesterday`)
              ) : (
                '—'
              )}
            </div>
          </div>

          {/* Active Alerts */}
          <div className="stat-card">
            <div className="stat-header">
              <h3>Active Alerts</h3>
            </div>
            <div className="stat-value">
              {alerts.length}
            </div>
            <div className="stat-label">System notifications</div>
          </div>
        </div>

        {/* Main Grid */}
        <div className="main-grid">
          {/* Left Column */}
          <div className="left-column">
            {/* Live Updates */}
            <div className="panel">
              <div className="panel-header dark">
                <h3>
                  <span className="pulse-indicator"></span>
                  Live Home Updates
                </h3>
              </div>
              <div className="panel-body">
                <div className="console">
                  {liveUpdates.length === 0 ? (
                    <div className="update-line muted">[sim] No updates yet. Waiting for device data...</div>
                  ) : (
                    <div>
                      <div className="update-line muted">Latest {liveUpdates.length} appliance updates</div>
                      {liveUpdates.map((u, i) => {
                        try {
                          const s = u.state || u;
                          const name = s.name || s.device || 'Unknown Device';
                          const watts = (s.watts !== undefined && s.watts !== null) ? ` (${s.watts}W)` : '';
                          const on = s.is_on ? 'ON' : 'OFF';
                          const statusClass = s.is_on ? 'status-on' : 'status-off';
                          const t = s.sim_time || s.real_time || u.sim_time || u.real_time;
                          const simDate = t ? new Date(t).toLocaleDateString() : '';
                          const simTime = t ? new Date(t).toLocaleTimeString() : '';
                          return (
                            <div key={i} className="update-line">
                              <span className="time">[sim {simDate} {simTime}]</span>{' '}
                              <span className="device-name">{name}</span>:{' '}
                              <span className={statusClass}>{on}</span>
                              <span className="watts">{watts}</span>
                            </div>
                          )
                        } catch (e) {
                          return <div key={i} className="update-line">{JSON.stringify(u)}</div>
                        }
                      })}
                    </div>
                  )}
                </div>
              </div>
            </div>

            {/* Predictions */}
            <div className="panel">
              <div className="panel-header">
                <h3>Usage & Cost Estimation</h3>
              </div>
              <div className="panel-body">
                <div className="prediction-form">
                  <div>
                    <label>Year</label>
                    <input type="number" id="pred-year" defaultValue={2026} min={2000} max={2100} />
                  </div>
                  <div>
                    <label>Month</label>
                    <input type="number" id="pred-month" defaultValue={2} min={1} max={12} />
                  </div>
                </div>
                <button className="btn primary btn-full" onClick={handlePrediction}>
                  Generate Estimate
                </button>

                {prediction && (
                  <div className="prediction-result">
                    <div className="small">
                      Forecast for {prediction.year}-{String(prediction.month).padStart(2, '0')}
                    </div>
                    <div className="prediction-line">
                      <div>
                        <div className="prediction-label">Estimated Usage</div>
                        <div className="prediction-value">
                          {prediction.monthly_kwh.toFixed(2)}
                          <span className="unit">kWh</span>
                        </div>
                      </div>
                      <div>
                        <div className="prediction-label">Estimated Cost</div>
                        <div className="prediction-value">
                          {prediction.monthly_cost.toFixed(2)}
                          <span className="unit">LKR</span>
                        </div>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </div>

            {/* Agent Chat */}
            <div className="panel">
              <div className="panel-header">
                <h3>AI Agent Chat</h3>
              </div>
              <div className="panel-body">
                <div className="chat-messages" ref={chatMessagesRef}>
                  {messages.length === 0 ? (
                    <div className="update-line muted">
                      Ask me anything about your energy usage...
                    </div>
                  ) : (
                    [...messages].reverse().map((m, i) => (
                      <div key={i} className={`chat-message ${m.from === 'You' ? 'user' : 'agent'}`}>
                        <div className="chat-bubble">
                          <span className="sender">{m.from}</span>
                          <div className="text">{m.text}</div>
                        </div>
                      </div>
                    ))
                  )}
                </div>
                <div className="chat-input-container">
                  <input 
                    value={chatInput} 
                    onChange={(e) => setChatInput(e.target.value)} 
                    onKeyDown={handleKey} 
                    placeholder="Type your message..." 
                  />
                  <button className="btn primary" onClick={sendMsg}>Send</button>
                </div>
              </div>
            </div>
          </div>

          {/* Right Column */}
          <div className="right-column">
            {/* Alerts */}
            <div className="panel">
              <div className="panel-header">
                <h3>Alerts & Warnings</h3>
              </div>
              <div className="panel-body">
                <div className="alerts-list">
                  {alerts.length === 0 ? (
                    <div className="empty-state">
                      <div className="empty-icon success">✓</div>
                      <p>All systems normal</p>
                    </div>
                  ) : (
                    alerts.map((a, i) => {
                      const sev = (a.severity || a.level || 'info').toString();
                      const cls = sev === 'error' ? 'alert-item error' : (sev === 'warn' ? 'alert-item warn' : 'alert-item info');
                      return <div key={i} className={cls}>{a.message}</div>
                    })
                  )}
                </div>
              </div>
            </div>

            {/* Recommendations */}
            <div className="panel">
              <div className="panel-header">
                <h3>Recommendations</h3>
              </div>
              <div className="panel-body">
                <div className="recommendation-list">
                  {recs.length === 0 ? (
                    <div className="empty-state">
                      <div className="empty-icon info">💡</div>
                      <p>No recommendations yet</p>
                    </div>
                  ) : (
                    recs.map((r, i) => {
                      const text = r.message || r.reply || r.text || JSON.stringify(r);
                      return <div key={i} className="recommendation-item">{text}</div>
                    })
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      </main>

      {/* Footer */}
      <footer>
        <p>Designed and developed by Akila Wanninayake under the course EEX6340 (BSE)</p>
      </footer>
    </div>
  )
}

export default App