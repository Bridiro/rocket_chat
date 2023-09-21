let roomListDiv = document.getElementById("room-list");
let messagesDiv = document.getElementById("messages");
let newMessageForm = document.getElementById("new-message");
let newRoomButton = document.getElementById("new-room-button");
let newRoomForm = document.getElementById("new-room");
let statusDiv = document.getElementById("status");
let popup = document.getElementById("popup");
let addRoomForm = document.getElementById("add-room");
let addRoomButton = document.getElementById("add-button");
let cancelPopupButton = document.getElementById("add-cancel");
let roomNameField = document.getElementById("new-room-name");
let roomDataList = document.getElementById("existing-rooms");
var newRoomPassword = document.getElementById("new-room-password");
var checkPassword = document.getElementById("check-password");

let roomTemplate = document.getElementById("room");
let messageTemplate = document.getElementById("message");

let messageField = newMessageForm.querySelector("#message");
let usernameField = newMessageForm.querySelector("#username");

var STATE = {
  room: "lobby",
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

  var node = roomTemplate.content.cloneNode(true);
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
  if (!STATE.rooms[name] || roomListDiv.querySelectorAll(".room").length <= 1) {
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

  if (roomListDiv.querySelectorAll(".room").length == 1) {
    roomListDiv
      .querySelector(`.room[data-name='${name}`)
      .classList.add("active");
  } else {
    var newRoom = roomListDiv.querySelector(`.room[data-name='${name}']`);
    var parentNewRoom = newRoom.parentElement;
    var newRoomRemove = parentNewRoom.querySelector(".remove-room");
    var oldRoom = roomListDiv.querySelector(`.room[data-name='${STATE.room}']`);
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
    var node = messageTemplate.content.cloneNode(true);
    node.querySelector(".message .username").textContent = username;
    node.querySelector(".message .username").style.color = hashColor(username);
    node.querySelector(".message .text").textContent = message;
    messagesDiv.appendChild(node);
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
      if (!"message" in msg || !"room" in msg || !"username" in msg) return;
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
      console.log(`connection lost. attempting to reconnect in ${timeout}s`);
      setTimeout(() => connect(uri), (() => timeout * 1000)());
    });
  }

  connect(uri);
}

// Set the connection status: `true` for connected, `false` for disconnected.
function setConnectedStatus(status) {
  STATE.connected = status;
  statusDiv.className = status ? "connected" : "reconnecting";
}

// Open popup
function openPopup() {
  popup.style.display = "block";
}

// Close popup
function closePopup() {
  popup.style.display = "none";
}

function cleanPopup() {
  newRoomPassword.disabled = true;
  checkPassword.checked = false;
  checkPassword.disabled = false;
  roomNameField.value = "";
  newRoomPassword.value = "";
}

// Let's go! Initialize the world.
function init() {
  addRoom("lobby");

  // Set up the form handler.
  newMessageForm.addEventListener("submit", (e) => {
    e.preventDefault();

    const room = STATE.room;
    const message = messageField.value;
    const username = usernameField.value || "guest";
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
  newRoomButton.addEventListener("click", (e) => {
    e.preventDefault();

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
            options += '<option value="' + room.room + '" >';
          });
          roomDataList.innerHTML = options;
        });
    }

    openPopup();
  });

  // Set up the add room handler
  addRoomForm.addEventListener("submit", (e) => {
    e.preventDefault();
    const require_password = checkPassword.checked;
    const room = roomNameField.value;
    const password = require_password ? newRoomPassword.value : "";
    const hidden = false;
    if (!room && !(require_password && password)) return;

    roomNameField.value = "";

    if (STATE.connected) {
      fetch("/add-room", {
        method: "POST",
        body: new URLSearchParams({ room, password, require_password, hidden }),
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
  cancelPopupButton.addEventListener("click", (e) => {
    e.preventDefault();

    cleanPopup();
    closePopup();
  });

  // Set up handler to able or disable password input from list
  roomNameField.addEventListener("input", (e) => {
    e.preventDefault();

    let roomName = roomNameField.value;
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
        checkPassword.disabled = false;
      }
    } else {
      addRoomButton.innerHTML = "Add";
    }
  });

  checkPassword.addEventListener("input", (e) => {
    e.preventDefault();
    if (checkPassword.checked) {
      newRoomPassword.disabled = false;
    } else {
      newRoomPassword.disabled = true;
    }
  });

  // Subscribe to server-sent events.
  subscribe("/events");
}

init();
