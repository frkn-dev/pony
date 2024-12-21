const zmq = require('zeromq');

async function runPublisher() {
    const sock = new zmq.Publisher();

    await sock.bind("tcp://127.0.0.1:3001");

    console.log("Publisher bound to port 3001");

    await new Promise(resolve => setTimeout(resolve, 2000));

     const createMessage = JSON.stringify({
        action: "create",
        user_id: "dc79e5c9-4b10-48b3-b7b8-534821ce48c7",
    });
    const msg = `mk3 ${createMessage}`;
    console.log("Sending create message:", msg);

    await sock.send(msg);


    
    
    await new Promise(resolve => setTimeout(resolve, 1000));
    const initMessage = JSON.stringify({
        action: "init",
        user_id: "23bd6e06-e98d-4081-a603-571eb266354d",
    });
    const initMsg = `mk3 ${initMessage}`;
    console.log("Sending create message:", initMsg);

    await sock.send(initMsg);

   // await new Promise(resolve => setTimeout(resolve, 1000));

   


}

runPublisher();
