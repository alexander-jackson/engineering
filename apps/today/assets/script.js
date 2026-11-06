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
};

document.addEventListener("DOMContentLoaded", onDocumentLoad);
