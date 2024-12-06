const zmq = require('zeromq');

async function runPublisher() {
    const sock = new zmq.Publisher();

    await sock.bind("tcp://127.0.0.1:3000");
    console.log("Publisher bound to port 3000");
    await new Promise(resolve => setTimeout(resolve, 1000));


    // Сообщение 1: Создание пользователя
    const createMessage = JSON.stringify({
        action: "create",
        user_id: "dc79e5c9-4b10-48b3-b7b8-534821ce48c7",
    });
    console.log("Sending create message:", createMessage);
    await sock.send(["dev", createMessage]);

    // Небольшая задержка перед следующим сообщением
    await new Promise(resolve => setTimeout(resolve, 2000));



    await new Promise(resolve => setTimeout(resolve, 1000));


    // Сообщение 1: Создание пользователя
    const createMessage2 = JSON.stringify({
        action: "create",
        user_id: "1ec1499c-c255-4d67-9d12-c5cd6c2a9a53",
        trial: false,
    });
    console.log("Sending create message:", createMessage2);
    await sock.send(["dev", createMessage2]);

    // Небольшая задержка перед следующим сообщением
    await new Promise(resolve => setTimeout(resolve, 2000));

     // Сообщение 1: Создание пользователя
    const createMessage3 = JSON.stringify({
        action: "create",
        user_id: "3747aefe-add3-4bad-badf-621e6585f3d0",
        trial: false,
        limit: 6000,
    });
    console.log("Sending create message:", createMessage3);
    await sock.send(["dev", createMessage3]);



   

  


}

runPublisher();
