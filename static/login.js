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

function resetUserError() {
    document.getElementById("username").classList.remove("errorField");
    let usernameLabel = document.querySelector('label[for="username"]');
    usernameLabel.classList.remove("errorLabel");
    usernameLabel.innerText = "Username";
}

function userError() {
    document.getElementById("username").classList.add("errorField");
    let usernameLabel = document.querySelector('label[for="username"]');
    usernameLabel.classList.add("errorLabel");
    usernameLabel.innerText += " | empty field!";
}

function resetPassError() {
    document.getElementById("password").classList.remove("errorField");
    let usernameLabel = document.querySelector('label[for="password"]');
    usernameLabel.classList.remove("errorLabel");
    usernameLabel.innerText = "Password";
}

function passError() {
    document.getElementById("password").classList.add("errorField");
    let passwordLabel = document.querySelector('label[for="password"]');
    passwordLabel.classList.add("errorLabel");
    passwordLabel.innerText += " | empty field!";
}

document.getElementById("username").addEventListener("input", () => {
    resetUserError();
});

document.getElementById("password").addEventListener("input", () => {
    resetPassError();
});

// Set up handler for the login form
document.querySelector("form").addEventListener("submit", (e) => {
    e.preventDefault();

    if (STATE.connected) {
        getPubKey();
        const userField = document.getElementById("username");
        const passField = document.getElementById("password");

        if (userField.value.trim() == "") {
            userError();
        }
        if (passField.value.trim() == "") {
            passError();
        }

        if (userField.value.trim() == "" || passField.value.trim() == "") {
            return;
        }

        const username = userField.value.trim();
        const password = encryptRsa(passField.value);

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
                    window.location.href = "/";
                }
            })
            .catch((err) => {
                console.error(err);
                userField.classList.add("errorField");
                passField.classList.add("errorField");
                let usernameLabel = document.querySelector(
                    'label[for="username"]'
                );
                usernameLabel.classList.add("errorLabel");
                usernameLabel.innerText += " | invalid user or password";
                document
                    .querySelector('label[for="password"]')
                    .classList.add("errorLabel");
            });
    }
});

document.getElementById("signup-button").addEventListener("click", (e) => {
    e.preventDefault();

    if (STATE.connected) {
        getPubKey();
        const userField = document.getElementById("username");
        const passField = document.getElementById("password");

        if (userField.value.trim() == "") {
            userError();
        }
        if (passField.value.trim() == "") {
            passError();
        }

        if (userField.value.trim() == "" || passField.value.trim() == "") {
            return;
        }

        const username = userField.value.trim();
        const password = encryptRsa(passField.value);

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
                    window.location.href = "/";
                }
            })
            .catch((err) => {
                console.error(err);
                userField.classList.add("errorField");
                passField.classList.add("errorField");
                let usernameLabel = document.querySelector(
                    'label[for="username"]'
                );
                usernameLabel.classList.add("errorLabel");
                usernameLabel.innerText += " | can't create user";
                document
                    .querySelector('label[for="password"]')
                    .classList.add("errorLabel");
            });
    }
});

subscribe("/events");
