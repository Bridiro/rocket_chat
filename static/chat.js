var STATE = {
    room: "lobby",
    user: "",
    rooms: {},
    connected: false,
};

// Generate a color from a "hash" of a string. Thanks, internet.
function hashColor(str) {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
        hash = str.charCodeAt(i) + ((hash << 5) - hash);
        hash = hash & hash;
    }

    return `hsl(${hash % 360}, 100%, 70%)`;
}

// Add a new room `name` and change to it. Returns `true` if the room didn't
// already exist and `false` otherwise.
function addRoom(name, key) {
    if (STATE.rooms[name]) {
        changeRoom(name);
        return false;
    }

    let roomListDiv = document.getElementById("room-list");
    let node = document.getElementById("room").content.cloneNode(true);
    let room = node.querySelector(".room");
    let button = node.querySelector(".remove-room");
    room.addEventListener("click", () => changeRoom(name));
    button.addEventListener("click", () => confirmRemoveRoom(name));
    room.value = name;
    room.dataset.name = name;
    roomListDiv.appendChild(node);

    STATE.rooms[name] = { key: key, messages: [] };
    changeRoom(name);
    return true;
}

function confirmRemoveRoom(room) {
    let popup = document.getElementById("confirm-remove");
    popup.querySelector("#room-name-remove").innerText = room;
    popup.style.display = "block";
}

function closeConfirmRemoveRoom() {
    let popup = document.getElementById("confirm-remove");
    popup.querySelector("#room-name-remove").innerText = "";
    popup.style.display = "none";
}

// Remove the room `name` and change to the first room available. Return `true`
// if the room was cancelled succesfully and `false` if it didn't exixsted
function removeRoom(name) {
    let roomListDiv = document.getElementById("room-list");
    if (
        !STATE.rooms[name] ||
        roomListDiv.querySelectorAll(".room").length <= 1
    ) {
        return false;
    }

    const room = name;
    const user = STATE.user;
    if (STATE.connected) {
        fetch("/remove-room", {
            method: "POST",
            body: new URLSearchParams({
                room,
                user,
            }),
        })
            .then((response) => {
                if (response.ok) {
                    let rooms = roomListDiv.querySelectorAll(".room");
                    if (
                        rooms[0].value == name &&
                        STATE.room == name &&
                        rooms.length > 1
                    )
                        changeRoom(rooms[1].value);
                    else if (STATE.room == name) changeRoom(rooms[0].value);

                    let node = roomListDiv.querySelector(
                        `.room[data-name='${name}']`
                    ).parentElement;
                    roomListDiv.removeChild(node);
                    delete STATE.rooms[name];
                    return true;
                } else {
                    return response.text().then((text) => {
                        throw new Error(text);
                    });
                }
            })
            .catch((err) => {
                console.error(err);
                return false;
            });
    }
}

// Change the current room to `name`, restoring its messages.
function changeRoom(name) {
    if (STATE.room == name) return;

    let roomListDiv = document.getElementById("room-list");
    let messagesDiv = document.getElementById("messages");
    if (roomListDiv.querySelectorAll(".room").length == 1) {
        roomListDiv
            .querySelector(`.room[data-name='${name}`)
            .classList.add("active");
    } else {
        let newRoom = roomListDiv.querySelector(`.room[data-name='${name}']`);
        let parentNewRoom = newRoom.parentElement;
        let newRoomRemove = parentNewRoom.querySelector(".remove-room");
        let oldRoom = roomListDiv.querySelector(
            `.room[data-name='${STATE.room}']`
        );
        let parentOldRoom = oldRoom.parentElement;
        let oldRoomRemove = parentOldRoom.querySelector(".remove-room");
        if (!newRoom || !oldRoom) return;

        oldRoom.classList.remove("active");
        newRoom.classList.add("active");
        oldRoomRemove.classList.remove("active");
        newRoomRemove.classList.add("active");
    }

    STATE.room = name;
    messagesDiv.querySelectorAll(".container-message").forEach((msg) => {
        messagesDiv.removeChild(msg);
    });

    STATE.rooms[name].messages.forEach((data) =>
        addMessage(name, data.username, data.message)
    );
}

// Add `message` from `username` to `room`. If `push`, then actually store the
// message. If the current room is `room`, render the message.
function addMessage(room, username, message, push = false) {
    if (push) {
        STATE.rooms[room].messages.push({ username, message });
    }

    if (STATE.room == room) {
        var node = document.getElementById("message").content.cloneNode(true);
        node.querySelector(".message .username").textContent = username;
        node.querySelector(".message .username").style.color =
            hashColor(username);
        node.querySelector(".message .text").textContent = message;
        if (username == STATE.user) {
            node.querySelector(".container-message").classList.add("minemess");
        }
        document.getElementById("messages").appendChild(node);
        setTimeout(scrollToBottom, 100);
    }
}

function scrollToBottom() {
    let chatContainer = document.getElementById("messages");
    chatContainer.scrollTop = chatContainer.scrollHeight;
}

// Subscribe to the event source at `uri` with exponential backoff reconnect.
function subscribe(uri) {
    let retryTime = 1;
    let done = false;

    function connect(uri) {
        const events = new EventSource(uri);

        events.addEventListener("message", (ev) => {
            const msg = JSON.parse(ev.data);
            if (!"message" in msg || !"room" in msg || !"username" in msg)
                return;
            if (STATE.rooms[msg.room])
                addMessage(
                    msg.room,
                    msg.username,
                    decryptAes(msg.message, STATE.rooms[msg.room].key),
                    true
                );
        });

        events.addEventListener("open", () => {
            setConnectedStatus(true);
            console.log(`connected to event stream at ${uri}`);
            getPubKey();
            if (!done) {
                setup();
                done = true;
            }
            retryTime = 1;
        });

        events.addEventListener("error", () => {
            setConnectedStatus(false);
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

// Set the connection status: `true` for connected, `false` for disconnected.
function setConnectedStatus(status) {
    STATE.connected = status;
    document.getElementById("status").className = status
        ? "connected"
        : "reconnecting";
}

// Open popup
function openRoomForm() {
    document.getElementById("add-room").style.display = "block";
}

// Close popup
function closeRoomForm() {
    document.getElementById("add-room").style.display = "none";
    document.getElementById("new-room-password").disabled = true;
    document.getElementById("check-password").checked = false;
    document.getElementById("check-password").disabled = false;
    document.getElementById("new-room-name").value = "";
    document.getElementById("new-room-password").value = "";
}

function encryptAes(message, key) {
    var b64 = CryptoJS.AES.encrypt(message, key).toString();
    var e64 = CryptoJS.enc.Base64.parse(b64);
    var eHex = e64.toString(CryptoJS.enc.Hex);
    return eHex;
}

function decryptAes(cipherText, key) {
    var reb64 = CryptoJS.enc.Hex.parse(cipherText);
    var bytes = reb64.toString(CryptoJS.enc.Base64);
    var decrypt = CryptoJS.AES.decrypt(bytes, key);
    var plain = decrypt.toString(CryptoJS.enc.Utf8);
    return plain;
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

function decryptRsa(encrypted) {
    const encryptedMessageBytes = forge.util.decode64(encrypted);
    const decryptedMessageBytes = STATE.clientKeys.privateKey.decrypt(
        encryptedMessageBytes
    );
    return forge.util.decodeUtf8(decryptedMessageBytes);
}

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

function getRooms() {
    const username = STATE.user;
    const rsa_key = forge.pki.publicKeyToPem(STATE.clientKeys.publicKey);
    fetch("/get-personal-rooms", {
        method: "POST",
        body: new URLSearchParams({ username, rsa_key }),
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
            const parsed = JSON.parse(data);
            if (parsed.length > 0) {
                parsed.forEach((room) => {
                    addRoom(room.room, decryptRsa(room.key));
                    room.messages.forEach((message) => {
                        addMessage(
                            message.room,
                            message.username,
                            decryptAes(
                                message.message,
                                STATE.rooms[message.room].key
                            ),
                            true
                        );
                    });
                });
                return;
            }
            return;
        })
        .catch((err) => {
            console.error(err);
        });
}

function setup() {
    fetch("/get-user", {
        method: "GET",
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
            STATE.user = data;
            document.title += " | " + STATE.user;
            getRooms();
        })
        .catch((err) => {
            console.error(err);
        });
}

function init() {
    // Generate RSA keys for the client
    STATE.clientKeys = forge.pki.rsa.generateKeyPair({ bits: 2048 });

    // Set up the handler to post a message.
    document.getElementById("new-message").addEventListener("submit", (e) => {
        e.preventDefault();

        const newMessageForm = document.getElementById("new-message");
        let messageField = newMessageForm.querySelector("#message");
        if (messageField.value.trim() == "") {
            return;
        }

        const room = STATE.room;
        const message = encryptAes(
            messageField.value.trim(),
            STATE.rooms[STATE.room].key
        );
        const username = STATE.user;
        if (!message || !username) return;

        if (STATE.connected) {
            fetch("/message", {
                method: "POST",
                body: new URLSearchParams({ room, username, message }),
            })
                .then((response) => {
                    if (response.ok) {
                        messageField.value = "";
                    } else {
                        return response.text().then((text) => {
                            throw new Error(text);
                        });
                    }
                })
                .catch((err) => {
                    console.error(err);
                });
        }
    });

    // Set up the open popup handler and get available rooms.
    document
        .getElementById("new-room-button")
        .addEventListener("click", (e) => {
            e.preventDefault();

            let roomDataList = document.getElementById("rooms-list");
            if (STATE.connected) {
                fetch("/get-rooms", {
                    method: "GET",
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
                        available_rooms = JSON.parse(data);
                        roomDataList.innerHTML = "";
                        available_rooms.forEach((room) => {
                            roomDataList.innerHTML +=
                                '<option value="' +
                                room.room +
                                '" data-rp="' +
                                room.require_password +
                                '" >';
                        });
                    })
                    .catch((err) => {
                        console.error(err);
                    });
            }

            openRoomForm();
        });

    // Set up the add room handler
    document.getElementById("add-room").addEventListener("submit", (e) => {
        e.preventDefault();

        if (STATE.connected) {
            getPubKey();
            const require_password =
                document.getElementById("check-password").checked;
            const room = document.getElementById("new-room-name").value.trim();
            const password = require_password
                ? encryptRsa(
                      document.getElementById("new-room-password").value.trim()
                  )
                : null;
            const hidden = false;
            const user = STATE.user;
            const rsa_client_key = forge.pki.publicKeyToPem(
                STATE.clientKeys.publicKey
            );
            if (room == "") {
                document.getElementById("new-room-name").value = "";
                document.getElementById("new-room-password").value = "";
                return;
            }

            fetch("/add-room", {
                method: "POST",
                body: new URLSearchParams({
                    room,
                    password,
                    require_password,
                    hidden,
                    user,
                    rsa_client_key,
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
                    const parsed = JSON.parse(data);
                    addRoom(parsed.room, decryptRsa(parsed.key));
                    parsed.messages.forEach((message) => {
                        addMessage(
                            message.room,
                            message.username,
                            decryptAes(
                                message.message,
                                STATE.rooms[message.room].key
                            ),
                            true
                        );
                    });
                    closeRoomForm();
                })
                .catch((err) => {
                    console.error(err);
                });
        }
    });

    // Set up the close popup handler
    document.getElementById("add-cancel").addEventListener("click", () => {
        closeRoomForm();
    });

    // Set up confirm remove room
    document
        .getElementById("confirm-remove")
        .addEventListener("submit", (e) => {
            e.preventDefault();
            let room = document.getElementById("room-name-remove").innerText;
            removeRoom(room);
            closeConfirmRemoveRoom();
        });

    // Set up cancel remove room
    document
        .getElementById("cancel-remove-button")
        .addEventListener("click", () => {
            closeConfirmRemoveRoom();
        });

    // Set up handler to able or disable password input from list
    document.getElementById("new-room-name").addEventListener("input", (e) => {
        e.preventDefault();

        let addRoomButton = document.getElementById("add-button");
        var newRoomPassword = document.getElementById("new-room-password");
        var checkPassword = document.getElementById("check-password");
        let roomName = document.getElementById("new-room-name").value;

        let options = document
            .getElementById("rooms-list")
            .querySelectorAll("option");
        for (var i = 0; i < options.length; i++) {
            if (roomName == options[i].value) {
                addRoomButton.innerHTML = "Join";
                if (options[i].dataset.rp == "true") {
                    newRoomPassword.disabled = false;
                    checkPassword.checked = true;
                    checkPassword.disabled = true;
                } else {
                    newRoomPassword.disabled = true;
                    checkPassword.disabled = true;
                    checkPassword.checked = false;
                }
                break;
            } else {
                addRoomButton.innerHTML = "Add";
                newRoomPassword.disabled = true;
                checkPassword.disabled = false;
                checkPassword.checked = false;
            }
        }
    });

    // Set up the handler to able or disable the password field based on the check
    document.getElementById("check-password").addEventListener("input", (e) => {
        e.preventDefault();
        if (document.getElementById("check-password").checked) {
            document.getElementById("new-room-password").disabled = false;
        } else {
            document.getElementById("new-room-password").disabled = true;
        }
    });

    document.getElementById("toggle-menu").addEventListener("click", () => {
        document.getElementById("sidebar").style.display = "flex";
    });

    document.addEventListener("click", (event) => {
        let sidebar = document.getElementById("sidebar");
        let toggleSidebarButton = document.getElementById("toggle-menu");
        if (window.matchMedia("(max-width: 992px)").matches) {
            if (
                !sidebar.contains(event.target) &&
                !toggleSidebarButton.contains(event.target)
            ) {
                sidebar.style.display = "none";
            }
        }
    });

    window.addEventListener("resize", () => {
        if (window.innerWidth > 992) {
            document.getElementById("sidebar").style.display = "flex";
        }
    });

    // Subscribe to server-sent events.
    subscribe("/events");
}

init();
