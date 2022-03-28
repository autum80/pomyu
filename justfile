deploy:
    rm -r pages || exit 0
    git worktree add -f pages pages
    trunk build --public-url /pomyu
    cd pages; git rm *
    cp -r dist/* pages
    cd pages; git add *
    cd pages; git commit -m "Update pages" || exit 0
    cd pages; git push -f origin pages
