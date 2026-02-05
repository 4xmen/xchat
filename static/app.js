function scrollToChat() {
    setTimeout(function () {
        const chatElement = document.querySelector('#chat');
        chatElement.scrollTop = chatElement.scrollHeight;
    }, 100);
}

function checkAndScrollToEnd() {
    const chatElement = document.querySelector('#chat');
    const isNearBottom = (chatElement.scrollHeight - chatElement.scrollTop - chatElement.clientHeight) < 200;

    if (isNearBottom) {
        scrollToChat();
    }
}


//set cookie
function setCookie(name, value, days) {
    const encodedValue = encodeURIComponent(value);
    const date = new Date();
    date.setTime(date.getTime() + (days * 24 * 60 * 60 * 1000));
    const expires = "expires=" + date.toUTCString();
    document.cookie = `${name}=${encodedValue};${expires};path=/`;
}

// read cookie
function getCookie(name) {
    const cookieName = name + "=";
    const decodedCookie = decodeURIComponent(document.cookie);
    const cookieArray = decodedCookie.split(';');

    for(let i = 0; i < cookieArray.length; i++) {
        let cookie = cookieArray[i];
        while (cookie.charAt(0) === ' ') {
            cookie = cookie.substring(1);
        }
        if (cookie.indexOf(cookieName) === 0) {
            // Using decodeURIComponent to decode the value
            return decodeURIComponent(cookie.substring(cookieName.length, cookie.length));
        }
    }
    return "";
}



var app = new Vue({
    el: "#app",
    data: {
        msg: "",
        msgList: [],
        canChat: false,
        name: "",
        ws: null,
    },
    methods: {
        sendMessage: function (e) {
            if (!this.canChat || this.ws == null) {
                this.handleWebSocket(this.name);
            } else {
                const t = this.msg.trim();
                if (t.length === 0) return;

                e.preventDefault();
                this.ws.send(JSON.stringify({ text: t }));
                this.msg = "";
            }
        },
        handleWebSocket: function (username) {
            if (username.trim().length == 0) {
                alert('plz enter name');
                return false;
            }
            try {
                const proto = location.protocol === "https:" ? "wss" : "ws";

                this.ws = new WebSocket(`${proto}://${location.host}/ws`);

                this.ws.onopen = () => {
                    this.ws.send(username); // first msg = username
                };

                this.ws.onmessage = (e) => {
                    try {
                        this.msgList.push(...JSON.parse(e.data));
                        checkAndScrollToEnd();
                    } catch (e) {
                        console.log(e.message);
                    }

                };
                if (!this.canChat) {
                    scrollToChat();
                    setCookie("username",this.name);
                    this.name = this.name.split("::")[0];
                }
                this.canChat = true;
            } catch (e) {
                console.log(e.message);
            }

        },
        async onFilePicked(e) {
            const file = e.target.files && e.target.files[0];
            if (!file) return;

            // if not connect first must be connect
            if (!this.canChat || !this.ws || this.ws.readyState !== 1) {
                this.handleWebSocket(this.name);
                try {
                    await this.waitForWsOpen();
                } catch (err) {
                    alert("WebSocket connection failed");
                    return;
                }
            }

            try {
                const fd = new FormData();
                fd.append("file", file);

                const res = await fetch("/upload", { method: "POST", body: fd });
                if (!res.ok) {
                    const t = await res.text();
                    alert("upload failed: " + t);
                    return;
                }

                const meta = await res.json();

                // msg with file (msg without file is allowed)
                this.ws.send(JSON.stringify({ text: "", attachment: meta }));
            } catch (err) {
                console.error(err);
                alert("upload failed");
            } finally {
                e.target.value = "";
            }
        },
        waitForWsOpen(timeoutMs = 5000) {
            return new Promise((resolve, reject) => {
                const start = Date.now();
                const tick = () => {
                    if (this.ws && this.ws.readyState === 1) return resolve();
                    if (Date.now() - start > timeoutMs) return reject(new Error("ws open timeout"));
                    setTimeout(tick, 50);
                };
                tick();
            });
        },
        fixTime: function (datetime) {
            let splited = datetime.split("T");
            return splited[0] + " " + splited[1].split('.')[0]
        },
        calcClass: function (msg) {
            let result = "";
            if (msg.username == this.name) {
                result = "own";
            }
            return result + ' ' + msg.type;
        }
    },
    mounted: function () {
        console.log('started vue app');
        this.name = getCookie("username")
    },
});