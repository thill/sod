#!/bin/bash

VERSION=$1

if [ -z "$VERSION" ]; then
  echo "usage: $0 {VERSION}"
  exit 1
fi

echo "setting workspace sod dependency to version=$VERSION ..."
sed -i "/sod = { path = \"sod\" }/c\sod = \"$VERSION\"" ./Cargo.toml

echo "setting workspace to version=$VERSION ..."
sed -i "/package.version = .*/c\package.version = \"$VERSION\"" ./Cargo.toml

echo "setting sod to version=$VERSION ..."
sed -i "/version = .*/c\version = \"$VERSION\"" ./sod/Cargo.toml

for PROJECT in sod-*; do
    echo "setting $PROJECT to version=$VERSION ..."
    sed -i "/version = .*/c\version = \"$VERSION\"" ./$PROJECT/Cargo.toml
    sed -i "/sod = { workspace = true }/c\sod = \"$VERSION\"" ./$PROJECT/Cargo.toml
done

echo "commiting changes ..."
git add .
git commit -m "release $VERSION"
git push

echo "publishing crates ..."
for PROJECT in sod*; do
    echo "publishing $PROJECT ..."
    pushd $PROJECT
    cargo publish
    popd
done

echo "preparing for next release ..."
sed -i "/sod = \"$VERSION\"/c\sod = { path = \"sod\" }" ./Cargo.toml
git add .
git commit -m "preparing for next release"
git push
