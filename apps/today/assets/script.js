const saveState = async (checkbox) => {
  const state = checkbox.checked ? "Checked" : "Unchecked";

  const response = await fetch(`/update/${checkbox.id}`, {
    method: "PATCH",
    body: JSON.stringify({ state }),
    headers: {
      "Content-Type": "application/json",
    },
  });

  if (!response.ok) {
    throw new Error(`failed to update item: ${response.status}`);
  }
};

const saveContent = async (uid, content) => {
  const response = await fetch(`/update/${uid}/content`, {
    method: "PATCH",
    body: JSON.stringify({ content }),
    headers: {
      "Content-Type": "application/json",
    },
  });

  if (!response.ok) {
    throw new Error(`failed to edit item: ${response.status}`);
  }
};

// Swaps the task text for an input holding its raw markdown
const editContent = (span, uid) => {
  const input = document.createElement("input");
  input.type = "text";
  input.value = span.dataset.source;
  input.className =
    "w-full bg-transparent text-xl leading-snug text-paper-ink border-b border-paper-ink focus:outline-none";

  // Stops blur or Escape from saving again once the edit has finished
  let finished = false;

  const cancel = () => {
    if (finished) return;

    finished = true;
    input.replaceWith(span);
  };

  const save = async () => {
    if (finished) return;

    const content = input.value.trim();

    // A blank or unchanged value is treated as cancelling the edit
    if (content === "" || content === span.dataset.source) {
      cancel();
      return;
    }

    finished = true;

    try {
      await saveContent(uid, content);
      // Reload so the rendered text comes from the server, as it does on first load
      window.location.reload();
    } catch (error) {
      console.error(error);
      input.replaceWith(span);
      window.alert("Could not save that change, so the task was left as it was.");
    }
  };

  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      save();
    } else if (event.key === "Escape") {
      cancel();
    }
  });

  input.addEventListener("blur", save);

  span.replaceWith(input);
  input.focus();
  input.select();
};

const onDocumentLoad = () => {
  const checkboxes = document.querySelectorAll("#items input[type='checkbox']");

  for (const checkbox of checkboxes) {
    checkbox.addEventListener("change", async () => {
      try {
        await saveState(checkbox);
      } catch (error) {
        // Items stay in place, so put the checkbox back to match what was saved
        checkbox.checked = !checkbox.checked;
        console.error(error);
      }
    });
  }

  const texts = document.querySelectorAll("#items span[data-source]");

  for (const span of texts) {
    const uid = span.closest("li").querySelector("input[type='checkbox']").id;

    span.addEventListener("click", () => editContent(span, uid));
  }
};

document.addEventListener("DOMContentLoaded", onDocumentLoad);
