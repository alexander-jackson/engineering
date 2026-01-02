interface TitleProps {
  value: string;
}

const Title = (props: TitleProps) => {
  const defaultClass = "py-3 m-0 text-center";

  return (
    <>
      <h2 className={`${defaultClass}`}>{props.value}</h2>
      <hr className="mt-0" />
    </>
  );
};

export default Title;
