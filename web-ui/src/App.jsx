import './App.css'
import aether_logo from './assets/aether_dark.png'
import { useState, useEffect } from "react";
// ReactDOM not used here; keep imports minimal

function App() {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [loggedIn, setLoggedIn] = useState(false)
  const [error, setError] = useState('')
  const [messages, setMessages] = useState([])
  const [msg, setMsg] = useState('')
  const [liveUpdates, setLiveUpdates] = useState([])
  function handleSubmit(e) {
    e.preventDefault()
    setError('')
    // Simple demo authentication: accept any non-empty username/password
    if (username.trim() === '' || password.trim() === '') {
      setError('Please enter username and password')
      return
    }

    // In a real app, call the backend /api/login here and store a token
    setLoggedIn(true)
  }

  function handleLogout() {
    setLoggedIn(false)
    setUsername('')
    setPassword('')
    setError('')
  }
  useEffect(() => {
    const url = `${location.protocol === 'https:' ? 'wss' : 'ws'}://127.0.0.1:3000/ws`;
    const ws = new WebSocket(url);

    ws.onopen = () => console.log('ws open');
    ws.onmessage = (evt) => {
      try {
        const obj = JSON.parse(evt.data);
        // push new state message onto live updates array
        setLiveUpdates(prev => [obj, ...prev].slice(0, 50));
      } catch(e) { console.error('ws message parse', e); }
    };
    ws.onclose = () => console.log('ws closed');

    return () => ws.close();
  }, []);

  if (!loggedIn) {
    return (
      <div className="login-container">
        <div className="login-card">
          <img src={aether_logo} alt="AETHER" className="logo" />
          <h2>Welcome to AETHER</h2>
          <h2>Agentic Energy Tracking System for Home Efficiency in Rust and React</h2>
          <div className="login-form-container">
            <form onSubmit={handleSubmit} className="login-form" style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '1rem'}}>
              <label>
                Username
                <input
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  type="text"
                  name="username"
                  autoComplete="username"
                  style={{marginLeft: "10px"}}
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
                  style={{marginLeft: "10px"}}
                />
              </label>

              {error && <div className="error">{error}</div>}

              <button type="submit" className="btn">Login</button>
            </form>
          </div>
        </div>
      </div>
    )
  }

  function sendMsg() {
    if (!msg.trim()) return
    const newMsg = { from: 'You', text: msg }
    setMessages(prev => [...prev, newMsg])
    setMsg('')
    // In real app: send via WebSocket to backend
  }

  return (
    <div className="app-root">
      <header className="app-header">
        <div className="brand">
          <img src={aether_logo} alt="AETHER" className="logo small" />
          <div>
            <h1>AETHER Dashboard</h1>
            <h2 style={{fontSize: '0.85rem', marginTop:2}}> An AI-based Energy Tracking System for Home Efficiency in Rust and React</h2>
          </div>
        </div>
        <div>
          <button onClick={handleLogout} className="btn secondary">Logout</button>
        </div>
      </header>

      <main className="dashboard">
        <section className="panel">
          <h3>Live Home Updates</h3>
          <div className="live-updates">
            <div className="console" role="log" aria-live="polite">
              {liveUpdates.length === 0 ? (
                <div className="update-line muted">[sim] No updates yet. Sim Time: --:--:--</div>
              ) : (() => {
                // Show all received appliance state messages (most recent first)
                const all = liveUpdates;
                return (
                  <div>
                    <div className="update-line muted">Latest {all.length} appliance updates</div>
                    {all.map((u, i) => {
                      try {
                        const s = u.state || u;
                        const name = s.name || s.device || 'unknown';
                        const watts = (s.watts !== undefined && s.watts !== null) ? ` (${s.watts}W)` : '';
                        const on = s.is_on ? 'ON' : 'OFF';
                        const t = s.sim_time || s.real_time || u.sim_time || u.real_time;
                        const simDate = t ? new Date(t).toLocaleDateString() : '';
                        const simTime = t ? new Date(t).toLocaleTimeString() : '';
                        return <div key={i} className="update-line">[sim {simDate} {simTime}] {name}: {on}{watts}</div>
                      } catch (e) {
                        return <div key={i} className="update-line">{JSON.stringify(u)}</div>
                      }
                    })}
                  </div>
                )
              })()}
            </div>
          </div>
        </section>

        <section className="panel alerts">
          <h3>Alerts & Warnings</h3>
          <div className="alert-item warn">Bulb left on during daylight hours</div>
          <div className="alert-item error">High peak usage detected yesterday</div>
        </section>

        <aside>
          <div className="panel wattage-monitor">
            <h3>Wattage Monitor</h3>
            <div>
              <p>Total current watts: <strong>1540 W</strong></p>
              <p>Average daily: <strong>2.1 kWh</strong></p>
            </div>
          </div>

          <div className="panel recommendations">
            <h3>Recommendations</h3>
            <ul>
              <li>Shift washing machine to off-peak hours.</li>
              <li>Lower AC thermostat by 2°C during peak hours.</li>
            </ul>
          </div>
        </aside>

        <section className="panel chat">
          <h3>Estimated Wattage & Cost</h3>
          <div style={{marginBottom:8}}>
            <label style={{marginRight:8}}>Year:
              <input type="number" id="pred-year" defaultValue={2026} style={{marginLeft:6, width:90}} />
            </label>
            <label>Month:
              <input type="number" id="pred-month" defaultValue={2} min={1} max={12} style={{marginLeft:6, width:60}} />
            </label>
            <button className="btn" style={{marginLeft:8}} onClick={async ()=>{
              const y = document.getElementById('pred-year').value
              const m = document.getElementById('pred-month').value
              try {
                // call backend prediction API
                const res = await fetch(`/api/predictions?year=${y}&month=${m}`)
                if (!res.ok) throw new Error(await res.text())
                const data = await res.json()
                // expected { monthly_kwh, monthly_cost, daily }
                setMessages([{from:'System', text:`Estimated ${y}-${m}: ${data.monthly_kwh} kWh, Cost LKR ${data.monthly_cost}`}])
              } catch (e) {
                setMessages([{from:'System', text:`Prediction failed: ${e.message}`}])
              }
            }}>Estimate</button>
          </div>
          <div className="chat-messages" id="chat-messages">
            {messages.length===0 ? <div><em>No estimates yet</em></div> : messages.map((m, i) => (
              <div key={i}><strong>{m.from}</strong>: {m.text}</div>
            ))}
          </div>
        </section>
      </main>
    </div>
  )
}

export default App
