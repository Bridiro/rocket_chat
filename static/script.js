var STATE = {
    room: "lobby",
    user: "",
    rooms: {},
    connected: false,
};

var AVAILABLE_ROOMS = {};

// Generate a color from a "hash" of a string. Thanks, internet.
function hashColor(str) {
    let hash = 0;
    for (var i = 0; i < str.length; i++) {
        hash = str.charCodeAt(i) + ((hash << 5) - hash);
        hash = hash & hash;
    }

    return `hsl(${hash % 360}, 100%, 70%)`;
}

// Add a new room `name` and change to it. Returns `true` if the room didn't
// already exist and `false` otherwise.
function addRoom(name) {
    if (STATE.rooms[name]) {
        changeRoom(name);
        return false;
    }

    let roomListDiv = document.getElementById("room-list");
    var node = document.getElementById("room").content.cloneNode(true);
    var room = node.querySelector(".room");
    var button = node.querySelector(".remove-room");
    room.addEventListener("click", () => changeRoom(name));
    button.addEventListener("click", () => removeRoom(name));
    room.textContent = name;
    room.dataset.name = name;
    roomListDiv.appendChild(node);

    STATE.rooms[name] = [];
    changeRoom(name);
    return true;
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

    let rooms = roomListDiv.querySelectorAll(".room");
    if (rooms[0].innerHTML == name && STATE.room == name && rooms.length > 1)
        changeRoom(rooms[1].innerHTML);
    else if (STATE.room == name) changeRoom(rooms[0].innerHTML);

    var node = roomListDiv.querySelector(
        `.room[data-name='${name}']`
    ).parentElement;
    roomListDiv.removeChild(node);
    delete STATE.rooms[name];
    return true;
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
        var newRoom = roomListDiv.querySelector(`.room[data-name='${name}']`);
        var parentNewRoom = newRoom.parentElement;
        var newRoomRemove = parentNewRoom.querySelector(".remove-room");
        var oldRoom = roomListDiv.querySelector(
            `.room[data-name='${STATE.room}']`
        );
        var parentOldRoom = oldRoom.parentElement;
        var oldRoomRemove = parentOldRoom.querySelector(".remove-room");
        if (!newRoom || !oldRoom) return;

        oldRoom.classList.remove("active");
        newRoom.classList.add("active");
        oldRoomRemove.classList.remove("active");
        newRoomRemove.classList.add("active");

        oldRoomRemove.style.display = "none";
        newRoomRemove.style.display = "inline";
    }

    STATE.room = name;
    messagesDiv.querySelectorAll(".message").forEach((msg) => {
        messagesDiv.removeChild(msg);
    });

    STATE.rooms[name].forEach((data) =>
        addMessage(name, data.username, data.message)
    );
}

// Add `message` from `username` to `room`. If `push`, then actually store the
// message. If the current room is `room`, render the message.
function addMessage(room, username, message, push = false) {
    if (push) {
        STATE.rooms[room].push({ username, message });
    }

    if (STATE.room == room) {
        var node = document.getElementById("message").content.cloneNode(true);
        node.querySelector(".message .username").textContent = username;
        node.querySelector(".message .username").style.color =
            hashColor(username);
        node.querySelector(".message .text").textContent = message;
        document.getElementById("messages").appendChild(node);
    }
}

// Subscribe to the event source at `uri` with exponential backoff reconnect.
function subscribe(uri) {
    var retryTime = 1;

    function connect(uri) {
        const events = new EventSource(uri);

        events.addEventListener("message", (ev) => {
            console.log("raw data", JSON.stringify(ev.data));
            console.log("decoded data", JSON.stringify(JSON.parse(ev.data)));
            const msg = JSON.parse(ev.data);
            if (!"message" in msg || !"room" in msg || !"username" in msg)
                return;
            addMessage(msg.room, msg.username, msg.message, true);
        });

        events.addEventListener("open", () => {
            setConnectedStatus(true);
            console.log(`connected to event stream at ${uri}`);
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
function openPopup() {
    document.getElementById("popup").style.display = "block";
}

// Close popup
function closePopup() {
    document.getElementById("popup").style.display = "none";
}

// Clean the popup fields
function cleanPopup() {
    document.getElementById("new-room-password").disabled = true;
    document.getElementById("check-password").checked = false;
    document.getElementById("check-password").disabled = false;
    document.getElementById("new-room-name").value = "";
    document.getElementById("new-room-password").value = "";
}

// CLose the login
function closeLogin() {
    document.getElementById("login").style.display = "none";
}

// Let's go! Initialize the world.
function init() {
    addRoom("lobby");

    // Set up handler for the login form
    document.getElementById("login-form").addEventListener("submit", (e) => {
        e.preventDefault();

        const username = document.getElementById("login-username").value;
        const password = document.getElementById("login-password").value;

        if (STATE.connected) {
            fetch("/login", {
                method: "POST",
                body: new URLSearchParams({
                    username,
                    password,
                }),
            })
                .then((response) => response.text())
                .then((data) => {
                    console.log(data);
                    if (data === "GRANTED") {
                        document.getElementById("login").style.display = "none";
                        STATE.user = username;
                        return;
                    }
                    return;
                });
        }
    });

    document.getElementById("sign-up-button").addEventListener("click", (e) => {
        e.preventDefault();

        const username = document.getElementById("login-username").value;
        const password = document.getElementById("login-password").value;

        if (STATE.connected) {
            fetch("/signup", {
                method: "POST",
                body: new URLSearchParams({
                    username,
                    password,
                }),
            })
                .then((response) => response.text())
                .then((data) => {
                    console.log(data);
                    if (data === "GRANTED") {
                        document.getElementById("login").style.display = "none";
                        STATE.user = username;
                        return;
                    }
                    return;
                });
        }
    });

    // Set up the handler to post a message.
    document.getElementById("new-message").addEventListener("submit", (e) => {
        e.preventDefault();

        const newMessageForm = document.getElementById("new-message");
        let messageField = newMessageForm.querySelector("#message");

        const room = STATE.room;
        const message = messageField.value;
        const username = STATE.user;
        console.log(room, message, username);
        if (!message || !username) return;

        if (STATE.connected) {
            fetch("/message", {
                method: "POST",
                body: new URLSearchParams({ room, username, message }),
            }).then((response) => {
                if (response.ok) messageField.value = "";
            });
        }
    });

    // Set up the open popup handler and get available rooms.
    document
        .getElementById("new-room-button")
        .addEventListener("click", (e) => {
            e.preventDefault();

            let roomDataList = document.getElementById("existing-rooms");
            if (STATE.connected) {
                fetch("/search-rooms", {
                    method: "POST",
                })
                    .then((response) => response.text())
                    .then((data) => {
                        AVAILABLE_ROOMS = JSON.parse(data);
                        console.log(AVAILABLE_ROOMS);
                        roomDataList.innerHTML = "";
                        var options = "";
                        AVAILABLE_ROOMS.forEach((room) => {
                            options += !room.hidden
                                ? '<option value="' + room.room + '" >'
                                : "";
                        });
                        roomDataList.innerHTML = options;
                    });
            }

            openPopup();
        });

    // Set up the add room handler
    document.getElementById("add-room").addEventListener("submit", (e) => {
        e.preventDefault();
        const require_password =
            document.getElementById("check-password").checked;
        const room = document.getElementById("new-room-name").value;
        const password = require_password
            ? document.getElementById("new-room-password").value
            : "";
        const hidden = false;
        const user = STATUS.user;
        if (room != "" && require_password == true && password === "") return;

        document.getElementById("new-room-name").value = "";

        if (STATE.connected) {
            fetch("/add-room", {
                method: "POST",
                body: new URLSearchParams({
                    room,
                    password,
                    require_password,
                    hidden,
                    user,
                }),
            })
                .then((response) => response.text())
                .then((data) => {
                    console.log(data);
                    if (data == "GRANTED") {
                        cleanPopup();
                        closePopup();
                        if (!addRoom(room)) return;
                    }
                    cleanPopup();
                    return;
                });
        }
    });

    // Set up the close popup handler
    document.getElementById("add-cancel").addEventListener("click", (e) => {
        e.preventDefault();
        AVAILABLE_ROOMS = {};
        cleanPopup();
        closePopup();
    });

    // Set up handler to able or disable password input from list
    document.getElementById("new-room-name").addEventListener("input", (e) => {
        e.preventDefault();

        let addRoomButton = document.getElementById("add-button");
        var newRoomPassword = document.getElementById("new-room-password");
        var checkPassword = document.getElementById("check-password");
        let roomName = document.getElementById("new-room-name").value;
        let exists = null;
        AVAILABLE_ROOMS.forEach((roomLoop) => {
            if (roomName == roomLoop.room) {
                exists = roomLoop;
            }
        });

        if (exists) {
            addRoomButton.innerHTML = "Join";
            if (exists.require_password) {
                newRoomPassword.disabled = false;
                checkPassword.checked = true;
                checkPassword.disabled = true;
            } else {
                newRoomPassword.disabled = true;
                checkPassword.disabled = true;
                checkPassword.checked = false;
            }
        } else {
            addRoomButton.innerHTML = "Add";
            newRoomPassword.disabled = true;
            checkPassword.disabled = false;
            checkPassword.checked = false;
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

    // Subscribe to server-sent events.
    subscribe("/events");
}

init();
