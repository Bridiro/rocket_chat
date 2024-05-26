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

function fieldError(m, f) {
    f.classList.add("errorField");
    let fLabel = document.querySelector('label[for="' + f.id + '"]');
    fLabel.classList.add("errorLabel");
    fLabel.innerText += " | " + m;
}

function fieldReset(f) {
    try {
        f.classList.remove("errorField");
        let fLabel = document.querySelector('label[for="' + f.id + '"]');
        fLabel.classList.remove("errorLabel");
        fLabel.innerText = fLabel.innerText.split(" | ")[0];
    } catch (error) {}
}

document.querySelector("form").addEventListener("submit", (e) => {
    e.preventDefault();

    if (STATE.connected) {
        getPubKey();
        const nameField = document.getElementById("fullname");
        const surnameField = document.getElementById("surname");
        const emailField = document.getElementById("email");
        const userField = document.getElementById("username");
        const passField = document.getElementById("password");
        const repeatField = document.getElementById("password-repeat");

        fieldReset(nameField);
        fieldReset(surnameField);
        fieldReset(emailField);
        fieldReset(userField);
        fieldReset(passField);
        fieldReset(repeatField);

        let err = false;

        if (nameField.value.trim() == "") {
            fieldError("Name is required", nameField);
            err = true;
        }
        if (surnameField.value.trim() == "") {
            fieldError("Surname is required", surnameField);
            err = true;
        }
        if (emailField.value.trim() == "") {
            fieldError("Email is required", emailField);
            err = true;
        }
        if (userField.value.trim() == "") {
            fieldError("Username is required", userField);
            err = true;
        }
        if (passField.value.trim() == "") {
            fieldError("Password is required", passField);
            err = true;
        }
        if (repeatField.value.trim() == "") {
            fieldError("Repeat password is required", repeatField);
            err = true;
        }
        if (passField.value.trim() != repeatField.value.trim()) {
            fieldError("Passwords don't match", repeatField);
            err = true;
        }

        if (err) {
            return;
        }

        const full_name = nameField.value.trim();
        const surname = surnameField.value.trim();
        const email = emailField.value.trim();
        const username = userField.value.trim();
        const password = encryptRsa(passField.value);

        fetch("/signup", {
            method: "POST",
            body: new URLSearchParams({
                full_name,
                surname,
                email,
                username,
                password,
            }),
        })
            .then((response) => {
                if (response.ok) {
                    location.href = "/";
                } else {
                    return response.text().then((text) => {
                        throw new Error(text);
                    });
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
                usernameLabel.innerText = "Username | can't create user";
                document
                    .querySelector('label[for="password"]')
                    .classList.add("errorLabel");
            });
    }
});

document.getElementById("login-button").addEventListener("click", () => {
    location.href = "/login";
});

subscribe("/events");
