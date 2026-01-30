import asyncio
import json
import os
from datetime import datetime, timezone
from typing import Set
from enum import StrEnum  # CRITICAL FIX FOR PYDANTIC ERROR

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse
from pydantic import BaseModel, Field
from dotenv import load_dotenv

# Load environment variables FIRST
load_dotenv()


class Role(StrEnum):  # Uses standard library StrEnum (Python 3.11+)
    ADMIN = "Admin"
    USER = "User"


class Member(BaseModel):
    role: Role
    name: str
    ip: str

class ChatMessage(BaseModel):
    text: str
    time: str  # ISO 8601 with 'Z' suffix
    username: str
    role: Role
    message_type: str = Field(..., alias="type")  # Avoids reserved keyword 'type'

    model_config = {  # Pydantic v2 config (replaces old Config class)
        "populate_by_name": True,
        "use_enum_values": True  # Serializes enums to their string values
    }


class AppState:
    def __init__(self):
        self.clients: Set[WebSocket] = set()
        self.messages: list[ChatMessage] = []

app_state = AppState()


def get_current_time() -> str:
    """Generate RFC3339 timestamp matching Rust's to_rfc3339()"""
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

def add_message(msg: ChatMessage):
    """Store non-system messages (max 50)"""
    if msg.username != "system":
        app_state.messages.append(msg)
        if len(app_state.messages) > 50:
            app_state.messages.pop(0)

async def broadcast_message(msg: ChatMessage):
    """Broadcast as JSON array to all clients"""
    if not msg.text.strip():
        return

    add_message(msg)
    
    # Serialize with alias ("type" instead of "message_type")
    payload = json.dumps([msg.model_dump(by_alias=True)])
    
    disconnected = set()
    for client in app_state.clients:
        try:
            await client.send_text(payload)
        except Exception:
            disconnected.add(client)
    
    app_state.clients -= disconnected


app = FastAPI(title="XChat", version="1.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)



@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    x_forwarded_for = websocket.headers.get("x-forwarded-for")
    ip = x_forwarded_for.split(",")[0].strip() if x_forwarded_for else (
        websocket.client.host if websocket.client else "unknown"
    )
    app_state.clients.add(websocket)
    member = None
    
    try:
        username = await websocket.receive_text()
        parts = username.split("::")
        member = Member(role=Role.USER, name=username, ip=ip)
        if member.name == "system":
            member.name = "[system]"
        
        token = os.getenv("TOKEN", "token")
        if len(parts) == 2 and parts[1] == token:
            member.role = Role.ADMIN
            member.name = parts[0]
        
        history = [msg.model_dump(by_alias=True) for msg in app_state.messages]
        await websocket.send_text(json.dumps(history))
        
        await broadcast_message(ChatMessage(
            text=f"{member.name} joined",
            time=get_current_time(),
            username="system",
            role=member.role,
            message_type="join"
        ))
        
        while True:
            data = await websocket.receive_text()
            await broadcast_message(ChatMessage(
                text=data,
                time=get_current_time(),
                username=member.name,
                role=member.role,
                message_type="message"
            ))
    except WebSocketDisconnect:
        pass
    finally:
        app_state.clients.discard(websocket)
        if member:
            await broadcast_message(ChatMessage(
                text=f"{member.name} left",
                time=get_current_time(),
                username="system",
                role=member.role,
                message_type="leave"
            ))


app.mount("/", StaticFiles(directory="static", html=True), name="static")


if __name__ == "__main__":
    import uvicorn
    port = int(os.getenv("PORT", "8000"))
    uvicorn.run("main:app", host="0.0.0.0", port=port, reload=True, log_level="info")