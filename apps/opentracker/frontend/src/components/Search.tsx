import Button from "react-bootstrap/Button";
import Fuse from "fuse.js";

interface Props {
  haystack: Array<string>;
  needle?: string;
  onClick: (value: string) => void;
}

const displayOptions = (
  options: Array<string>,
  onClick: (value: string) => void,
) => {
  return (
    <div className="my-1">
      <p className="text-muted mb-1">Suggestions</p>
      <div>
        {options.map((e, i) => (
          <Button
            variant="primary"
            className="m-2"
            key={i}
            onClick={() => onClick(e)}
          >
            {e}
          </Button>
        ))}
      </div>
    </div>
  );
};

const Search = (props: Props) => {
  const { haystack, needle, onClick } = props;

  // If we have no haystack, there's nothing to show
  if (haystack.length === 0) {
    return null;
  }

  if (!needle) {
    return displayOptions(haystack, onClick);
  }

  const fuse = new Fuse(haystack, { includeScore: true });
  const results = fuse.search(needle);

  // Check whether we have an exact match
  if (results.find((i) => i.score === 0)) {
    return null;
  }

  // Check whether we have any suggestions to make
  if (!results.length) {
    return <p className="text-muted mb-1">No Suggestions</p>;
  }

  return displayOptions(
    results.map((e) => e.item),
    onClick,
  );
};

export default Search;
