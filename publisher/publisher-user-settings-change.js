const zmq = require('zeromq');

async function runPublisher() {
    const sock = new zmq.Publisher();

    await sock.bind("tcp://127.0.0.1:3000");
    console.log("Publisher bound to port 3000");
    await new Promise(resolve => setTimeout(resolve, 1000));


    // Сообщение 2: Изменение trial на false
    const updateTrialMessage = JSON.stringify({
        action: "update",
        user_id: "dc79e5c9-4b10-48b3-b7b8-534821ce48c7",
        trial: false,
    });
    console.log("Sending update trial message:", updateTrialMessage);
    await sock.send(["dev", updateTrialMessage]);

    // Еще одна задержка
    await new Promise(resolve => setTimeout(resolve, 2000));

    // Сообщение 3: Изменение лимита на 2000
    const updateLimitMessage = JSON.stringify({
        action: "update",
        user_id: "dc79e5c9-4b10-48b3-b7b8-534821ce48c7",
        limit: 2000,
    });
    console.log("Sending update limit message:", updateLimitMessage);
    await sock.send(["dev", updateLimitMessage]);


}

runPublisher();
