const zmq = require('zeromq');

async function runPublisher() {
    const sock = new zmq.Publisher();

    await sock.bind("tcp://127.0.0.1:3000");
    console.log("Publisher bound to port 3000");

    //const actions = ["create", "delete", "update"];
    //const tags = ["vmess", "vless", "shadowsocks"];
    const tags = ["vmess"];
    const actions = ["create", "update"];
    
    
    const messages = [];
    for (let i = 0; i < 150; i++) {
        const action = actions[i % actions.length]; 
        const user_id = `user_${i + 1}`;
        const tag = tags[i % tags.length]; 
        const trial = i % 2 === 0; 

        
        const message = JSON.stringify({ tag, action, user_id, trial, limit: 9999 });
        messages.push([tag, message]);
    }

   
    let i = 0;
    setInterval(() => {
        const message = messages[i];
        console.log("Sending:", message[1]);
        sock.send(["dev", message[1]]);

        i++;
        if (i === messages.length) {
            i = 0; // Перезапуск отправки сообщений
        }
    }, 1000);
}

runPublisher();
