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
                if (this.msg.trim().length == 0) {
                    return;
                }
                e.preventDefault();
                this.ws.send(this.msg.trim());
                this.msg = "";
            }
        },
        handleWebSocket: function (username) {
            if (username.trim().length == 0) {
                alert('plz enter name');
                return false;
            }
            try {
                this.ws = new WebSocket(`ws://${location.host}/ws`);

                this.ws.onopen = () => {
                    this.ws.send(username); // اولین پیام = username
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
                    this.name = this.name.split("::")[0];
                }
                this.canChat = true;
            } catch (e) {
                console.log(e.message);
            }

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
    },
});