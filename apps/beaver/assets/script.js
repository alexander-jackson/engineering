// Submits the query with Cmd or Ctrl + Enter, as Enter alone adds a new line
document.addEventListener("DOMContentLoaded", () => {
  const form = document.getElementById("query-form");
  const query = document.getElementById("query");

  query.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      form.requestSubmit();
    }
  });
});
