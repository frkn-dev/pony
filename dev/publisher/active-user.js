
const zmq = require('zeromq');

async function runPublisher(user_id) {
    const sock = new zmq.Publisher();

    await sock.bind("tcp://127.0.0.1:3000");

    console.log("Publisher bound to port 3000");

    await new Promise(resolve => setTimeout(resolve, 2000));

   const createMessage = JSON.stringify({
    action: "create",
    conn_id: user_id,
    tag: "Wireguard",
    wg: {
        keys: {
            "pubkey": "4KWw7RSRFTuy5q/RrSTv0k7z+Ym66bm++R1Js3ucOFM=",
            "privkey": "oF/N/YrFUC7v6Xq2NLIVJNyurgmmqU2I7rOCm9gkLUo="
        },
        "address": {
  "ip": "10.10.10.100",
  "cidr": 32
}

        
    }
}); 

    const msg = `dev ${createMessage}`;
    console.log("Sending create message:", msg);

    await sock.send(msg);
}

const userId = process.argv[2];  

if (userId) {
    runPublisher(userId);
} else {
    console.log("Please provide a user_id as the second argument.");
}
