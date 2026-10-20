#!/usr/bin/env python3
"""Read only a committed sibling component into this component's ignored scratch."""
import argparse
import io
from pathlib import Path
import subprocess
import tarfile


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--worktree',type=Path,required=True)
    parser.add_argument('--component',choices=['core','qt','tauri'],required=True)
    args=parser.parse_args()
    def git(*parts):
        return subprocess.check_output(['git','-C',str(args.worktree),*parts])
    revision=git('rev-parse','HEAD').decode().strip()
    component=f'prototypes/overnight/{args.component}'
    data=git('archive',revision,component)
    output=Path(__file__).resolve().parent/'.scratch'/'snapshots'/args.component/revision
    output.mkdir(parents=True,exist_ok=False)
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        archive.extractall(output,filter='data')
    print(f'{revision}\n{output/component}')


if __name__=='__main__':
    main()
