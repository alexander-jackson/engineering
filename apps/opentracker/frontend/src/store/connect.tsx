import { connect as libconnect } from "react-redux";

import { RootState, AppDispatch } from "~/store";

interface StateMapper<T> {
  (state: RootState): T;
}

const mapDispatch = (dispatch: AppDispatch) => {
  return { dispatch };
};

type DispatchProps = ReturnType<typeof mapDispatch>;

const connect = <State, Props>(mapState: StateMapper<State>) => {
  type StateProps = ReturnType<typeof mapState>;

  const mergeProps = (
    stateProps?: StateProps,
    dispatchProps?: DispatchProps,
    ownProps?: Props,
  ): StateProps & DispatchProps & Props => {
    return Object.assign({}, stateProps, dispatchProps, ownProps);
  };

  return libconnect(mapState, mapDispatch, mergeProps);
};

export default connect;
