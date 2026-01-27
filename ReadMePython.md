# XChat - Real‑time WebSocket Chat With Python

A lightweight real‑time chat application with admin/user roles and message history.

## 📋 Prerequisites
- Python 3.11 or newer (verified with Python 3.14)
- Basic command line knowledge
- Web browser (Chrome, Firefox, Edge)

## ⚙️ Required Files Structure
```text
xchat/
├── main.py          # Application backend
├── .env             # Configuration file
└── static/
├── index.html   # Chat interface
├── app.js       # Frontend logic
├── app.css      # Styling
└── vue.min.js   # Vue.js 2 library (must be version 2.x)
```

## 🚀 Setup & Run Instructions

1. **Install required packages**  
```powershell
pip install fastapi uvicorn python-dotenv pydantic
```
2. **Create .env file (in project root)**
```.env
TOKEN=your_secret_token_here
PORT=8000
```
- Replace your_secret_token_here with your actual admin token.

3. **Download Vue.js 2 (critical requirement)**

```powershell
curl https://cdn.jsdelivr.net/npm/vue@2.6.14/dist/vue.min.js -o static/vue.min.js
```

4. **Start the server**

```powershell
python main.py
```

5. **Open in browser**
- Visit: http://localhost:8000