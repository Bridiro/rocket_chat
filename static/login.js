var STATE = {
    connected: false,
};

// Generate RSA keys for the client
clientKeys = forge.pki.rsa.generateKeyPair({ bits: 2048 });

function getPubKey() {
    if (STATE.connected) {
        fetch("/rsa-pub-key", {
            method: "GET",
        })
            .then((response) => response.text())
            .then((data) => {
                STATE.serverPubKey = data;
            });
    }
}

// Subscribe to the event source at `uri` with exponential backoff reconnect.
function subscribe(uri) {
    var retryTime = 1;

    function connect(uri) {
        const events = new EventSource(uri);

        events.addEventListener("open", () => {
            STATE.connected = true;
            console.log(`connected to event stream at ${uri}`);
            getPubKey();
            retryTime = 1;
        });

        events.addEventListener("error", () => {
            STATE.connected = false;
            events.close();

            let timeout = retryTime;
            retryTime = Math.min(64, retryTime * 2);
            console.log(
                `connection lost. attempting to reconnect in ${timeout}s`
            );
            setTimeout(() => connect(uri), (() => timeout * 1000)());
        });
    }

    connect(uri);
}

function encryptRsa(message) {
    try {
        return forge.util.encode64(
            forge.pki
                .publicKeyFromPem(STATE.serverPubKey)
                .encrypt(forge.util.encodeUtf8(message))
        );
    } catch (error) {
        console.log("Errore durante la crittografia: ", error);
    }
}

// Set up handler for the login form
document.getElementById("login-form").addEventListener("submit", (e) => {
    e.preventDefault();

    if (STATE.connected) {
        getPubKey();
        const username = document.getElementById("login-username").value;
        const password = encryptRsa(
            document.getElementById("login-password").value
        );

        fetch("/login", {
            method: "POST",
            body: new URLSearchParams({
                username,
                password,
            }),
        })
            .then((response) => {
                if (response.ok) {
                    return response.text();
                } else {
                    return response.text().then((text) => {
                        throw new Error(text);
                    });
                }
            })
            .then((data) => {
                console.log(data);
                if (data === "GRANTED") {
                    window.location.href = "/chat";
                }
            })
            .catch((err) => {
                console.error(err);
            });
    }
});

document.getElementById("sign-up-button").addEventListener("click", (e) => {
    e.preventDefault();

    if (STATE.connected) {
        getPubKey();
        const username = document.getElementById("login-username").value;
        const password = encryptRsa(
            document.getElementById("login-password").value
        );

        fetch("/signup", {
            method: "POST",
            body: new URLSearchParams({
                username,
                password,
            }),
        })
            .then((response) => {
                if (response.ok) {
                    return response.text();
                } else {
                    return response.text().then((text) => {
                        throw new Error(text);
                    });
                }
            })
            .then((data) => {
                console.log(data);
                if (data === "GRANTED") {
                    window.location.href = "/chat";
                }
            })
            .catch((err) => {
                console.error(err);
            });
    }
});

subscribe("/events");
