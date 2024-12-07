const zmq = require('zeromq');

async function runPublisher() {
    const sock = new zmq.Publisher();

    await sock.bind("tcp://127.0.0.1:3000");
    console.log("Publisher bound to port 3000");
    await new Promise(resolve => setTimeout(resolve, 1000));


    // Небольшая задержка перед следующим сообщением
    await new Promise(resolve => setTimeout(resolve, 2000));

   //// Сообщение 3: Изменение expired
   const user = JSON.stringify({
      action: "restore",
      user_id: "dc79e5c9-4b10-48b3-b7b8-534821ce48c7",
   });
   console.log("Sending Active message:", user);
   await sock.send(["dev", user]);
   await new Promise(resolve => setTimeout(resolve, 2000));

   // Еще одна задержка
    await new Promise(resolve => setTimeout(resolve, 2000));

    


}

runPublisher();
