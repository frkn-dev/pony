### node active-user.js $(xray uuid)

const zmq = require('zeromq');

async function runPublisher(user_id) {
    const sock = new zmq.Publisher();

    await sock.bind("tcp://127.0.0.1:3001");

    console.log("Publisher bound to port 3001");

    await new Promise(resolve => setTimeout(resolve, 2000));

    const createMessage = JSON.stringify({
        action: "create",
        user_id: user_id,
    });

    const msg = `mk5 ${createMessage}`;
    console.log("Sending create message:", msg);

    await sock.send(msg);
}

const userId = process.argv[2];  

if (userId) {
    runPublisher(userId);
} else {
    console.log("Please provide a user_id as the second argument.");
}
