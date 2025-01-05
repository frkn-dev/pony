

# Important 

The SNI needs to be carefully chosen for hiding Vless traffic, taking into account location and government regulations


## Check domain availability 
   
    curl -I https://cdn.discordapp.com

Should return ``` 200 OK.```


## Check SNI (Server Name Indication)

    openssl s_client -connect cdn.discordapp.com:443 -servername cdn.discordapp.com

If success - good to use

## Check ALPN

    openssl s_client -connect cdn.discordapp.com:443 -alpn h2,http/1.1

    Should show available protocols



## Check available sub domains 

```
DOMAIN=discordapp.com
while read sub; do
  echo "Checking $sub.$DOMAIN:"
  curl -I -w "%{http_code}\n" -s "https://$sub.$DOMAIN" | head -n 1
done < sub.txt
```


```
www
mail
webmail
ftp
api
cdn
static
blog
forum
shop
admin
cpanel
dashboard
login
portal
en
ru
es
fr
de
dev
test
staging
beta
demo
help
support
docs
status
about
news
media
download
secure
vault
```
