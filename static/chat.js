/* trunk-ignore-all(prettier) */
var STATE = {
    recipient_id: -1,
    room_id: -1,
    user_id: -1,
    user: "",
    rooms: {},
    users: {},
    connected: false,
};

var ws = null;

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
function addRoom(id, name, key) {
    if (STATE.rooms[id]) {
        changeRoom(id);
        return false;
    }

    let roomListDiv = document.getElementById("room-list");
    let node = document.getElementById("room").content.cloneNode(true);
    let room = node.querySelector(".room");
    let button = node.querySelector(".remove-room");
    room.addEventListener("click", () => changeRoom(id));
    button.addEventListener("click", () => confirmRemoveRoom(name));
    room.value = name;
    room.dataset.name = name;
    room.dataset.id = id;
    roomListDiv.appendChild(node);

    STATE.rooms[id] = { name: name, key: key, messages: [] };
    changeRoom(id);
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
    let id = roomListDiv.querySelector(`.room[data-name='${name}']`).dataset.id;
    if (!STATE.rooms[id] || roomListDiv.querySelectorAll(".room").length <= 1) {
        return false;
    }

    const room_id = id;
    const user_id = STATE.user_id;
    if (STATE.connected) {
        fetch("/remove-room", {
            method: "POST",
            body: new URLSearchParams({
                room_id,
                user_id,
            }),
        })
            .then((response) => {
                if (response.ok) {
                    let rooms = roomListDiv.querySelectorAll(".room");
                    if (
                        rooms[0].dataset.id == id &&
                        STATE.room_id == id &&
                        rooms.length > 1
                    )
                        changeRoom(rooms[1].dataset.id);
                    else if (STATE.room_id == id)
                        changeRoom(rooms[0].dataset.id);

                    let node = roomListDiv.querySelector(
                        `.room[data-id='${id}']`
                    ).parentElement;
                    roomListDiv.removeChild(node);
                    delete STATE.rooms[id];
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
function changeRoom(id) {
    if (STATE.room_id == id) return;

    let roomListDiv = document.getElementById("room-list");
    let messagesDiv = document.getElementById("messages");
    if (roomListDiv.querySelectorAll(".room").length == 1) {
        roomListDiv
            .querySelector(`.room[data-id='${id}']`)
            .classList.add("active");
    } else {
        let newRoom = roomListDiv.querySelector(`.room[data-id='${id}']`);
        let parentNewRoom = newRoom.parentElement;
        let newRoomRemove = parentNewRoom.querySelector(".remove-room");
        let oldRoom = roomListDiv.querySelector(
            `.room[data-id='${STATE.room_id}']`
        );
        let parentOldRoom = oldRoom.parentElement;
        let oldRoomRemove = parentOldRoom.querySelector(".remove-room");
        if (!newRoom || !oldRoom) return;

        oldRoom.classList.remove("active");
        newRoom.classList.add("active");
        oldRoomRemove.classList.remove("active");
        newRoomRemove.classList.add("active");
    }

    STATE.room_id = id;
    messagesDiv.querySelectorAll(".container-message").forEach((msg) => {
        messagesDiv.removeChild(msg);
    });

    STATE.rooms[id].messages.forEach((data) =>
        addMessageGroup(id, data.user_id, data.username, data.message)
    );
}

function addUser(id, name, key) {
    if (STATE.users[id]) {
        changeUser(id);
        return false;
    }

    let userListDiv = document.getElementById("user-list");
    let node = document.getElementById("user").content.cloneNode(true);
    let user = node.querySelector(".user");
    let button = node.querySelector(".remove-user");

    user.addEventListener("click", () => changeUser(id));
    button.addEventListener("click", () => confirmRemoveUser(name));
    user.value = name;
    user.dataset.name = name;
    user.dataset.id = id;
    userListDiv.appendChild(node);

    STATE.users[id] = { name: name, key: key, messages: [] };
    changeUser(id);
    return true;
}

function removeUser(name) {
    let userListDiv = document.getElementById("room-list");
    let id = userListDiv.querySelector(`.room[data-name='${name}']`).dataset.id;
    if (!STATE.users[id] || userListDiv.querySelectorAll(".user").length <= 1) {
        return false;
    }

    const recipient_id = id;
    const user_id = STATE.user_id;
    if (STATE.connected) {
        fetch("/remove-direct", {
            method: "POST",
            body: new URLSearchParams({
                recipient_id,
                user_id,
            }),
        })
            .then((response) => {
                if (response.ok) {
                    let users = userListDiv.querySelectorAll(".user");
                    if (
                        users[0].dataset.id == id &&
                        STATE.recipient_id == id &&
                        users.length > 1
                    )
                        changeUser(users[1].dataset.id);
                    else if (STATE.recipient_id == id)
                        changeRoom(users[0].dataset.id);

                    let node = userListDiv.querySelector(
                        `.user[data-id='${id}']`
                    ).parentElement;
                    userListDiv.removeChild(node);
                    delete STATE.users[id];
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

function changeUser(id) {
    if (STATE.recipient_id == id) return;

    let userListDiv = document.getElementById("user-list");
    let messagesDiv = document.getElementById("messages");
    if (userListDiv.querySelectorAll(".user").length == 1) {
        userListDiv
            .querySelector(`.user[data-id='${id}']`)
            .classList.add("active");
    } else {
        let newUser = userListDiv.querySelector(`.user[data-id='${id}']`);
        let parentNewUser = newUser.parentElement;
        let newUserRemove = parentNewUser.querySelector(".remove-user");
        let oldUser = userListDiv.querySelector(
            `.user[data-id='${STATE.recipient_id}']`
        );
        let parentOldUser = oldUser.parentElement;
        let oldUserRemove = parentOldUser.querySelector(".remove-user");
        if (!newUser || !oldUser) return;

        oldUser.classList.remove("active");
        newUser.classList.add("active");
        oldUserRemove.classList.remove("active");
        newUserRemove.classList.add("active");
    }

    STATE.recipient_id = id;
    messagesDiv.querySelectorAll(".container-message").forEach((msg) => {
        messagesDiv.removeChild(msg);
    });

    STATE.users[id].messages.forEach((data) =>
        addMessageDirect(id, data.sender_id, data.message)
    );
}

function swapRoomsToUsersMessages() {
    let messagesDiv = document.getElementById("messages");
    messagesDiv.querySelectorAll(".container-message").forEach((msg) => {
        messagesDiv.removeChild(msg);
    });

    STATE.users[STATE.recipient_id].messages.forEach((data) =>
        addMessageDirect(STATE.recipient_id, data.sender_id, data.message)
    );
}

function swapUsersToRoomMessages() {
    let messagesDiv = document.getElementById("messages");
    messagesDiv.querySelectorAll(".container-message").forEach((msg) => {
        messagesDiv.removeChild(msg);
    });

    STATE.rooms[STATE.room_id].messages.forEach((data) =>
        addMessageGroup(
            STATE.room_id,
            data.user_id,
            data.username,
            data.message
        )
    );
}

// Add `message` from `username` to `room`. If `push`, then actually store the
// message. If the current room is `room`, render the message.
function addMessageDirect(chat_id, sender_id, message, push = false) {
    if (push) {
        STATE.users[chat_id].messages.push({ sender_id, message });
    }

    if (STATE.recipient_id == chat_id) {
        var node = document.getElementById("direct").content.cloneNode(true);
        node.querySelector(".message .text").textContent = message;
        if (sender_id == STATE.user_id) {
            node.querySelector(".container-message").classList.add("minemess");
        }
        document.getElementById("messages").appendChild(node);
        setTimeout(scrollToBottom, 100);
    }
}

// Add `message` from `username` to `room`. If `push`, then actually store the
// message. If the current room is `room`, render the message.
function addMessageGroup(room_id, _user_id, username, message, push = false) {
    if (push) {
        STATE.rooms[room_id].messages.push({ username, message });
    }

    if (STATE.room_id == room_id) {
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

    function connect(uri) {
        const events = new EventSource(uri);

        events.addEventListener("message", (ev) => {
            const msg = JSON.parse(ev.data);
            if (
                !"message" in msg ||
                !"room_id" in msg ||
                !"user_id" in msg ||
                !"user_name" in msg
            )
                return;
            if (STATE.rooms[msg.room_id])
                addMessageGroup(
                    msg.room_id,
                    msg.user_id,
                    msg.user_name,
                    decryptAes(msg.message, STATE.rooms[msg.room_id].key),
                    true
                );
        });

        events.addEventListener("open", () => {
            setConnectedStatus(true);
            console.log(`connected to event stream at ${uri}`);
            getPubKey();
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

function openUserForm() {
    document.getElementById("add-user").style.display = "block";
}

function closeUserForm() {
    document.getElementById("new-user-name").value = "";
    document.getElementById("add-user").style.display = "none";
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
    const user_id = STATE.user_id;
    const rsa_key = forge.pki.publicKeyToPem(STATE.clientKeys.publicKey);
    fetch("/get-personal-rooms", {
        method: "POST",
        body: new URLSearchParams({ user_id, rsa_key }),
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
                    addRoom(room.room_id, room.room_name, decryptRsa(room.key));
                    room.messages.forEach((message) => {
                        addMessageGroup(
                            message.room_id,
                            message.user_id,
                            message.user_name,
                            decryptAes(
                                message.message,
                                STATE.rooms[message.room_id].key
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

function getDirects() {
    const user_id = STATE.user_id;
    const rsa_key = forge.pki.publicKeyToPem(STATE.clientKeys.publicKey);
    fetch("/get-directs", {
        method: "POST",
        body: new URLSearchParams({ user_id, rsa_key }),
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
                parsed.forEach((direct) => {
                    console.log(direct.user_id);
                    addUser(
                        direct.user_id,
                        direct.user_name,
                        decryptRsa(direct.key)
                    );
                    direct.messages.forEach((message) => {
                        addMessageDirect(
                            direct.user_id,
                            message.user_id,
                            decryptAes(
                                message.message,
                                STATE.users[direct.user_id].key
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
    fetch("/whoami", {
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
            data = JSON.parse(data);
            STATE.user_id = data.id;
            STATE.user = data.username;
            document.title += " | " + STATE.user;

            ws = new WebSocket(
                "ws://" + location.host + "/messages/" + STATE.user_id
            );

            ws.onmessage = (event) => {
                const msg = JSON.parse(event.data);
                if ("Group" in msg) {
                    addMessageGroup(
                        msg.Group.group_id,
                        msg.Group.sender_id,
                        msg.Group.sender_name,
                        decryptAes(
                            msg.Group.content,
                            STATE.rooms[msg.Group.group_id].key
                        ),
                        (document.getElementById("room-list").style.display =
                            "block" ? true : false)
                    );
                } else if ("Direct" in msg) {
                    addMessageDirect(
                        msg.Direct.sender,
                        msg.Direct.sender,
                        decryptAes(
                            msg.Direct.content,
                            STATE.users[msg.Direct.sender].key
                        ),
                        (document.getElementById("user-list").style.display =
                            "block" ? true : false)
                    );
                } else {
                    console.error("unknown message type");
                }
            };

            getRooms();
            getDirects();
        })
        .catch((err) => {
            console.error(err);
        });
    return true;
}

function swapUsersRooms(i) {
    if (i == 1) {
        document.getElementById("user-list").style.display = "block";
        document.getElementById("room-list").style.display = "none";
        swapRoomsToUsersMessages();
    } else if (i == 2) {
        document.getElementById("user-list").style.display = "none";
        document.getElementById("room-list").style.display = "block";
        swapUsersToRoomMessages();
    }
}

function init() {
    document.getElementById("user-list").style.display = "block";
    // Generate RSA keys for the client
    STATE.clientKeys = forge.pki.rsa.generateKeyPair({ bits: 2048 });

    setup();

    setTimeout(() => {
        swapUsersRooms(2);
        swapUsersRooms(1);
    }, 1000);

    const radioButtons = document.querySelectorAll('input[name="options"]');

    radioButtons.forEach((radio) => {
        radio.addEventListener("change", () => {
            if (radio.checked) {
                swapUsersRooms(radio.dataset.toggle);
            }
        });
    });

    // Set up the handler to post a message.
    document.getElementById("new-message").addEventListener("submit", (e) => {
        e.preventDefault();

        const newMessageForm = document.getElementById("new-message");
        let messageField = newMessageForm.querySelector("#message");
        if (messageField.value.trim() == "") {
            return;
        }

        if (document.getElementById("user-list").style.display == "block") {
            const sender = STATE.user_id;
            const recipient = STATE.recipient_id;
            const content = encryptAes(
                messageField.value.trim(),
                STATE.users[STATE.recipient_id].key
            );
            if (!content) return;

            ws.send(
                '{ "Direct" :' +
                    JSON.stringify({ sender, recipient, content }) +
                    " }"
            );
            addMessageDirect(
                recipient,
                sender,
                decryptAes(content, STATE.users[recipient].key),
                true
            );
        } else if (
            document.getElementById("room-list").style.display == "block"
        ) {
            const sender_id = STATE.user_id;
            const sender_name = STATE.user;
            const group_id = STATE.room_id;
            const content = encryptAes(
                messageField.value.trim(),
                STATE.rooms[STATE.room_id].key
            );
            if (!content || !sender_name) return;

            ws.send(
                '{ "Group" :' +
                    JSON.stringify({
                        sender_id,
                        sender_name,
                        group_id,
                        content,
                    }) +
                    " }"
            );
            addMessageGroup(
                group_id,
                sender_id,
                sender_name,
                decryptAes(content, STATE.rooms[group_id].key),
                true
            );
        } else {
            console.error("unknown message type");
        }

        messageField.value = "";
    });

    // Set up the open popup handler and get available rooms.
    document
        .getElementById("new-room-button")
        .addEventListener("click", (e) => {
            e.preventDefault();

            if (document.getElementById("room-list").style.display == "block") {
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
                                    room.room_name +
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
            } else {
                openUserForm();
            }
        });

    // Set up the add room handler
    document.getElementById("add-room").addEventListener("submit", (e) => {
        e.preventDefault();

        if (STATE.connected) {
            getPubKey();
            const require_password =
                document.getElementById("check-password").checked;
            const room_name = document
                .getElementById("new-room-name")
                .value.trim();
            const password = require_password
                ? encryptRsa(
                      document.getElementById("new-room-password").value.trim()
                  )
                : null;
            const hidden = false;
            const user_id = STATE.user_id;
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
                    room_name,
                    password,
                    require_password,
                    hidden,
                    user_id,
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
                    addRoom(
                        parsed.room_id,
                        parsed.room_name,
                        decryptRsa(parsed.key)
                    );
                    parsed.messages.forEach((message) => {
                        addMessageGroup(
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

    document.getElementById("add-user").addEventListener("submit", (e) => {
        e.preventDefault();

        const user_id = STATE.user_id;
        const username = document.getElementById("new-user-name").value.trim();
        const rsa_key = forge.pki.publicKeyToPem(STATE.clientKeys.publicKey);
        if (username == "") {
            document.getElementById("new-user-name").value = "";
            return;
        }

        fetch("/add-direct", {
            method: "POST",
            body: new URLSearchParams({ user_id, username, rsa_key }),
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
                addUser(parsed.id, username, decryptRsa(parsed.key));
                closeUserForm();
            })
            .catch((err) => {
                console.error(err);
            });
    });

    document.getElementById("add-user-cancel").addEventListener("click", () => {
        closeUserForm();
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

    document.getElementById("user-menu").addEventListener("click", () => {
        document.getElementById("user-form").style.display = "block";
    });

    document.getElementById("cancel-password").addEventListener("click", () => {
        document.getElementById("old-password").value = "";
        document.getElementById("new-password").value = "";
        document.getElementById("confirm-password").value = "";
        document.getElementById("user-form").style.display = "none";
    });

    document.getElementById("logout").addEventListener("click", () => {
        fetch("/logout", {
            method: "GET",
        });
        location.href = "/login";
    });

    document.getElementById("user-form").addEventListener("submit", (e) => {
        e.preventDefault();

        let old_password_input = document.getElementById("old-password");
        let new_password_input = document.getElementById("new-password");
        let repeat_password_input = document.getElementById("confirm-password");

        let user_id = STATE.user_id;
        let old_password = old_password_input.value.trim();
        let new_password = new_password_input.value.trim();
        let repeat_password = repeat_password_input.value.trim();

        if (
            old_password == "" ||
            new_password == "" ||
            repeat_password == "" ||
            new_password != repeat_password ||
            old_password == new_password
        ) {
            return;
        }

        old_password = encryptRsa(old_password);
        new_password = encryptRsa(new_password);

        fetch("/change-pass", {
            method: "POST",
            body: new URLSearchParams({
                user_id,
                old_password,
                new_password,
            }),
        }).then((response) => {
            if (response.ok) {
                document.getElementById("user-form").style.display = "none";
            } else {
                alert("Errore nel modificare la password!");
            }
        });
    });

    document.addEventListener("click", (event) => {
        let sidebar = document.getElementById("sidebar");
        let toggleSidebarButton = document.getElementById("toggle-menu");
        let addRoomForm = document.getElementById("add-room");
        let confirmRemoveForm = document.getElementById("confirm-remove");
        if (window.matchMedia("(max-width: 992px)").matches) {
            if (
                !sidebar.contains(event.target) &&
                !toggleSidebarButton.contains(event.target) &&
                !addRoomForm.contains(event.target) &&
                !confirmRemoveForm.contains(event.target)
            ) {
                sidebar.style.display = "none";
            }
        }
    });

    window.addEventListener("resize", () => {
        if (window.innerWidth > 992) {
            document.getElementById("sidebar").style.display = "flex";
        } else {
            document.getElementById("sidebar").style.display = "none";
        }
    });

    // Subscribe to server-sent events.
    subscribe("/events");
}

init();
