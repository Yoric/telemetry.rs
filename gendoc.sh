#!/bin/sh

echo "Preparing to release latest documentation."

latest=`git show -s --oneline HEAD`
git checkout gh-pages
git pull --rebase origin master

echo "Generating documentation"
cargo doc

echo "Exporting documentation"
cp -R target/doc/* doc/latest

echo "Pushing"
git commit -am "Doc bump: $latest"
git pull --rebase origin gh-pages
git push origin gh-pages
git checkout master
