console.log("start js script");

var test = 0;

async function asyncCall() {
    while (true) {
        test++;
        console.log("update " + test);
    }
}

async function connect() {
    // const connect = await Deno.connect({ hostname: "slint.dev", port: 80 });
    console.log("test");
}

connect();

// asyncCall();
