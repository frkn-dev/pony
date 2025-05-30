const zmq = require('zeromq');

async function runPublisher() {
    const sock = new zmq.Publisher();

    await sock.bind("tcp://127.0.0.1:3000");
    console.log("Publisher bound to port 3000");
    await new Promise(resolve => setTimeout(resolve, 1000));

    const createMessage = JSON.stringify({
        action: "create",
        user_id: "ff9aafb1-1102-417b-874c-42418f414c1b",
        password: "TEST",
});
    console.log("Sending create message:", createMessage);
    await sock.send(["dev", createMessage]);

    await new Promise(resolve => setTimeout(resolve, 2000));


}

runPublisher();
