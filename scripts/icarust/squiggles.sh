REFERENCE=$1
OUTDIR=$2

micromamba shell init -s bash -p ~/micromamba
source ~/.bashrc

if [ ! -d "$HOME/micromamba/envs/icarust" ]; then
    micromamba create -f /usr/src/icarust/python/icarust.yaml -y
fi

micromamba activate icarust

mkdir -p $OUTDIR

python /usr/src/icarust/python/make_squiggle.py $REFERENCE --out_dir $OUTDIR